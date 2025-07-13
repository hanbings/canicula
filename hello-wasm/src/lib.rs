#![no_std]
#![no_main]

#[link(wasm_import_module = "host")]
extern "C" {
    fn hello(x: i32) -> i32;
}

#[no_mangle]
pub extern "C" fn call_host() {
    unsafe {
        hello(42);
    }
}

// panic handler 是必须的
use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
