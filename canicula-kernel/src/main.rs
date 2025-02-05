#![no_std]
#![no_main]

use noto_sans_mono_bitmap::{get_raster, FontWeight, RasterHeight};

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
    let frame_buffer = boot_info.framebuffer.as_mut().unwrap();

    let buffer = frame_buffer.buffer_mut().as_ptr() as *mut u32;
    let width = frame_buffer.info().width;
    let height = frame_buffer.info().height;

    for index in 0..(width * height) {
        unsafe {
            buffer.add(index as usize).write(0xff408deb);
        }
    }

    let msg = "Hello World";
    draw_string(
        msg,
        400,
        300,
        width as usize,
        height as usize,
        FontWeight::Regular,
        RasterHeight::Size16,
        unsafe { core::slice::from_raw_parts_mut(buffer, (width * height) as usize) },
    );

    loop {}
}

fn draw_string(
    msg: &str,
    x: u32,
    y: u32,
    width: usize,
    _height: usize,
    font_weight: FontWeight,
    raster_height: RasterHeight,
    draw_buffer: &mut [u32],
) {
    for (char_i, char) in msg.chars().enumerate() {
        let char_raster = get_raster(char, font_weight, raster_height).expect("unknown char");
        for (row_i, row) in char_raster.raster().iter().enumerate() {
            for (col_i, intensity) in row.iter().enumerate() {
                let index = char_i * char_raster.width()
                    + col_i
                    + row_i * width
                    + (x as usize)
                    + (y as usize * width);

                let curr_pixel_rgb = draw_buffer[index];
                let mut r = ((curr_pixel_rgb & 0xff0000) >> 16) as u8;
                let mut g = ((curr_pixel_rgb & 0xff00) >> 8) as u8;
                let mut b = (curr_pixel_rgb & 0xff) as u8;

                r = r.saturating_add(*intensity);
                g = g.saturating_add(*intensity);
                b = b.saturating_add(*intensity);

                let new_pixel_rgb = ((r as u32) << 16) + ((g as u32) << 8) + (b as u32);

                draw_buffer[index] = new_pixel_rgb;
            }
        }
    }
}
