#![no_std]
#![no_main]
#![allow(dead_code)]
#![cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    feature(abi_x86_interrupt)
)]
#![feature(alloc_error_handler)]

mod arch;
mod resources;
mod types;

#[unsafe(no_mangle)]
#[cfg(target_arch = "riscv64")]
pub fn kernel() -> ! {
    arch::riscv::entry();
}

#[unsafe(no_mangle)]
#[cfg(target_arch = "aarch64")]
pub fn kernel() -> ! {
    arch::aarch::entry();
}

#[unsafe(no_mangle)]
#[cfg(target_arch = "x86_64")]
pub fn kernel_main(boot_info: &'static mut canicula_common::entry::BootInfo) -> ! {
    arch::x86::entry(boot_info)
}
