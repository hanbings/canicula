#![no_std]
#![no_main]
#![feature(panic_info_message)]

mod arch;

#[no_mangle]
#[cfg(target_arch = "riscv64")]
pub fn kernel() -> ! {
    arch::riscv::entry();
}

#[no_mangle]
#[cfg(target_arch = "aarch64")]
pub fn kernel() -> ! {
    arch::aarch::entry();
}

/// This is the entry point for the x86-64 (UEFI) kernel.
#[no_mangle]
#[cfg(target_arch = "x86_64")]
pub fn kernel(_boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    arch::x86::entry();
}
