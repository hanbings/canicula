use core::panic::PanicInfo;

use crate::{error, info};

mod console;
mod interrupts;
mod serial;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {:#?}", info);

    loop {}
}

pub struct X86Arch {
    pub(crate) boot_info: &'static mut bootloader_api::BootInfo,
}

impl crate::arch::Arch for X86Arch {
    fn entry(&mut self) -> ! {
        let frame_buffer = self.boot_info.framebuffer.as_mut().unwrap();

        crate::arch::x86::console::init(frame_buffer);
        crate::arch::x86::interrupts::init();

        info!("This is the x86_64 kernel message");

        loop {}
    }
}
