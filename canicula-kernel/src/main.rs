#![no_std]
#![no_main]

mod arch;
mod config;
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
    use canicula_common::console::NotoFontDisplay;
    use noto_sans_mono_bitmap::{FontWeight, RasterHeight};

    let frame_buffer = boot_info.framebuffer.as_mut().unwrap();

    let buffer = frame_buffer.buffer_mut().as_ptr() as *mut u32;
    let width = frame_buffer.info().width;
    let height = frame_buffer.info().height;

    for index in 0..(width * height) {
        unsafe {
            buffer.add(index as usize).write(0xff408deb);
        }
    }

    let mut console = NotoFontDisplay::new(
        width as usize,
        height as usize,
        unsafe { core::slice::from_raw_parts_mut(buffer, (width * height) as usize) },
        FontWeight::Regular,
        RasterHeight::Size16,
    );

    let msg = "Hello World";

    for _index in 0..100 {
        console.draw_string(msg);
    }

    loop {}
}
