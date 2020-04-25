use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, Mutex};
use crate::memory::region::{Region, U8Access, U16Access, U32Access, U64Access, BytesAccess};
use std::ops::Deref;
use std::fmt::{Display, Formatter};
use std::fmt;
use std::ops::Bound::{Included, Unbounded};

#[derive(Debug)]
pub enum Error {
    Overlap(String, String),
    Renamed(String, String),
}


struct RegionCPtr(*const Box<Arc<Region>>);

unsafe impl Send for RegionCPtr {}

unsafe impl Sync for RegionCPtr {}


//Space should be an owner of Regions
pub struct Space {
    regions: BTreeMap<u64, (String, Arc<Region>)>,
    //for ffi free
    ptrs: HashMap<String, Vec<RegionCPtr>>,
}

impl Space {
    pub fn new() -> Space {
        Space { regions: BTreeMap::new(), ptrs: HashMap::new() }
    }

    pub fn add_region(&mut self, name: &str, region: &Arc<Region>) -> Result<Arc<Region>, Error> {
        let check = || {
            if let Some(_) = self.regions.values().find(|(n, _)| { n == name }) {
                return Err(Error::Renamed(name.to_string(), format!("region name {} has existed!", name)));
            }
            if let Some(v) = self.regions.values().find(|(_, v)| {
                region.info.base >= v.info.base && region.info.base < v.info.base + v.info.size ||
                    region.info.base + region.info.size - 1 >= v.info.base && region.info.base + region.info.size - 1 < v.info.base + v.info.size ||
                    v.info.base >= region.info.base && v.info.base < region.info.base + region.info.size ||
                    v.info.base + v.info.size - 1 >= region.info.base && v.info.base + v.info.size - 1 < region.info.base + region.info.size
            }) {
                return Err(Error::Overlap(v.0.to_string(), format!("region [{} : {:?}] is overlapped with [{} : {:?}]!", name, region.deref().info, v.0, v.1.deref().info)));
            }
            Ok(())
        };
        check()?;
        self.regions.insert(region.info.base, (name.to_string(), Arc::clone(region)));
        Ok(Arc::clone(region))
    }

    pub fn delete_region(&mut self, name: &str) {
        let res = self.regions.iter().find_map(|(k, (n, _))| { if n == name { Some(*k) } else { None } });
        if let Some(k) = res {
            self.regions.remove(&k);
        }
        if let Some(ps) = self.ptrs.remove(name) {
            ps.iter().for_each(|RegionCPtr(ptr)| { std::mem::drop(unsafe { (*ptr).read() }) })
        }
    }

    pub fn get_region(&self, name: &str) -> Option<Arc<Region>> {
        if let Some(v) = self.regions.values().find_map(|(n, region)| { if n == name { Some(region) } else { None } }) {
            Some(Arc::clone(v))
        } else {
            None
        }
    }

    pub fn get_region_by_addr(&self, addr: u64) -> Result<Arc<Region>, u64> {
        if let Some((_, (_, v))) = self.regions.range((Unbounded,Included(&addr))).last() {
            if addr < v.info.base + v.info.size {
                Ok(Arc::clone(v))
            } else {
                Err(addr)
            }
        } else {
            Err(addr)
        }
    }

    pub fn write_u8(&self, addr: u64, data: u8) -> Result<(), u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(U8Access::write(region.deref(), addr, data))
    }

    pub fn read_u8(&self, addr: u64) -> Result<u8, u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(U8Access::read(region.deref(), addr))
    }

    pub fn write_u16(&self, addr: u64, data: u16) -> Result<(), u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(U16Access::write(region.deref(), addr, data))
    }

    pub fn read_u16(&self, addr: u64) -> Result<u16, u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(U16Access::read(region.deref(), addr))
    }

    pub fn write_u32(&self, addr: u64, data: u32) -> Result<(), u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(U32Access::write(region.deref(), addr, data))
    }

    pub fn read_u32(&self, addr: u64) -> Result<u32, u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(U32Access::read(region.deref(), addr))
    }

    pub fn write_u64(&self, addr: u64, data: u64) -> Result<(), u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(U64Access::write(region.deref(), addr, data))
    }

    pub fn read_u64(&self, addr: u64) -> Result<u64, u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(U64Access::read(region.deref(), addr))
    }

    fn write_bytes(&self, addr: u64, data: &[u8]) -> Result<(), u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(BytesAccess::write(region.deref(), addr, data))
    }

    fn read_bytes(&self, addr: u64, data: &mut [u8]) -> Result<(), u64> {
        let region = self.get_region_by_addr(addr)?;
        Ok(BytesAccess::read(region.deref(), addr, data))
    }

    pub fn clean(&mut self, name: &str, ptr: *const Box<Arc<Region>>) {
        self.ptrs.entry(String::from(name)).or_insert(vec![])
            .push(RegionCPtr(ptr))
    }
}

impl Display for Space {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "regions:")?;
        for (name, region) in self.regions.values() {
            writeln!(f, "   {:<10}({:^13})  : {:#016x} -> {:#016x}", name, region.get_type(), region.info.base, region.info.base + region.info.size - 1)?;
        }
        Ok(())
    }
}

lazy_static! {
    pub static ref SPACE_TABLE:SpaceTable = SpaceTable { spaces: Mutex::new(HashMap::new()) };
}

pub struct SpaceTable {
    spaces: Mutex<HashMap<String, Arc<Mutex<Space>>>>,
}

impl SpaceTable {
    pub fn get_space(&self, name: &str) -> Arc<Mutex<Space>> {
        let mut map = self.spaces.lock().unwrap();
        map.entry(String::from(name))
            .or_insert_with(|| {
                if name == "space_query" {
                    println!("create space_query")
                }
                Arc::new(Mutex::new(Space::new()))
            }).clone()
    }
}
