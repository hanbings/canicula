#![no_std]
#![no_main]

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

const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 100 * 1024;
    config
};
bootloader_api::entry_point!(kernel_main, config = &CONFIG);

fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    let frame_buffer = boot_info.framebuffer.as_mut().unwrap();

    let buffer = frame_buffer.buffer_mut().as_ptr() as *mut u32;
    let width = frame_buffer.info().width;
    let height = frame_buffer.info().height;

    for index in 0..(width * height) {
        unsafe {
            buffer.add(index as usize).write(0xff408deb);
        }
    }

    loop {}
}
