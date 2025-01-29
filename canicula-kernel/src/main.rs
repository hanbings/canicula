#![no_std]
#![no_main]

use canicula_efi::BootInfo;

canicula_efi::entry_point!(kernel);

pub fn kernel(boot_info: &'static BootInfo) -> ! {
    let frame_buffer = boot_info.graphic_info.fb_addr;

    let buffer = frame_buffer as *mut u32;
    for index in 0..boot_info.graphic_info.fb_size / 4 {
        unsafe {
            buffer.add(index as usize).write(0xff408deb);
        }
    }

    loop {}
}
