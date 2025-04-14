#![no_std]
#![no_main]
#![allow(dead_code)]
#![cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    feature(abi_x86_interrupt)
)]
#![feature(alloc_error_handler)]

mod arch;
mod types;
mod resources;

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

#[cfg(target_arch = "x86_64")]
const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 1000 * 1024;
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};
#[cfg(target_arch = "x86_64")]
bootloader_api::entry_point!(kernel_main, config = &CONFIG);

#[no_mangle]
#[cfg(target_arch = "x86_64")]
fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    arch::x86::entry(boot_info)
}
