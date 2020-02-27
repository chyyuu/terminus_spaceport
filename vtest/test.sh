cd ..
./build.sh
cd -
verilator --cc --exe -sv -o $PWD/test --vpi \
--top-module TestModule verilator_main.cc test.v +incdir+$PWD/../vsrc \
$PWD/../target/release/libdpi_memory.1.so \
-CFLAGS -DVERILATOR -CFLAGS -fPIC -CFLAGS -I$PWD/../csrc -CFLAGS -I$PWD \
-LDFLAGS -Wl,-rpath=$PWD/../target/release
make -C obj_dir -f VTestModule.mk
./test
rm test
rm -rf obj_dir