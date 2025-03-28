#![no_std]
#![no_main]

mod arch;
mod types;

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
    config.kernel_stack_size = 100 * 1024;
    config
};
#[cfg(target_arch = "x86_64")]
bootloader_api::entry_point!(kernel_main, config = &CONFIG);

#[no_mangle]
#[cfg(target_arch = "x86_64")]
fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    use arch::Arch;

    arch::x86::X86Arch { boot_info }.entry();
}
