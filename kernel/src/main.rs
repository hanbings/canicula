#![no_std]
#![no_main]
#![feature(panic_info_message)]

mod arch;

#[no_mangle]
pub fn rust_main() -> ! {
    #[cfg(target_arch = "riscv64")]
    arch::riscv::entry();

    #[cfg(target_arch = "x86_64")]
    arch::x86_64::entry();
}
