use core::panic::PanicInfo;

use qemu::exit_qemu;

use crate::{error, info};

mod console;
mod gdt;
mod interrupts;
mod qemu;
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
        crate::arch::x86::console::init(self.boot_info.framebuffer.as_mut().unwrap());
        crate::arch::x86::interrupts::init();
        crate::arch::x86::gdt::init();

        info!("This is the x86_64 kernel message");

        self.boot_info.memory_regions.iter().for_each(|region| {
            info!(
                "Memory region: {:#x} - {:#x} ({:#x} bytes), type: {:?}",
                region.start,
                region.end,
                region.end - region.start,
                region.kind
            );
        });

        exit_qemu(0x10)
    }
}
