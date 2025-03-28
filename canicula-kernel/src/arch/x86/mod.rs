use core::panic::PanicInfo;

use crate::println;
use crate::serial_println;

mod console;
mod interrupts;
mod serial;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

pub struct X86Arch {
    pub(crate) boot_info: &'static mut bootloader_api::BootInfo,
}

impl crate::arch::Arch for X86Arch {
    fn entry(&mut self) -> ! {
        use noto_sans_mono_bitmap::{FontWeight, RasterHeight};

        let frame_buffer = self.boot_info.framebuffer.as_mut().unwrap();

        let buffer = frame_buffer.buffer_mut().as_ptr() as *mut u32;
        let width = frame_buffer.info().width;
        let height = frame_buffer.info().height;

        for index in 0..(width * height) {
            unsafe {
                buffer.add(index as usize).write(0xff408deb);
            }
        }        

        crate::arch::x86::console::init(
            width as usize,
            height as usize,
            unsafe { core::slice::from_raw_parts_mut(buffer, (width * height) as usize) },
            FontWeight::Regular,
            RasterHeight::Size16,
        );
        crate::arch::x86::interrupts::init();

        println!("This is the x86_64 kernel");
        serial_println!("This is the x86_64 kernel");

        loop {}
    }
}
