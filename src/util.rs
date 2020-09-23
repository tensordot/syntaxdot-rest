use std::os::raw::c_int;

#[allow(dead_code)]
#[no_mangle]
extern "C" fn mkl_serv_intel_cpu_true() -> c_int {
    1
}
