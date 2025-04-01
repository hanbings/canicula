use core::panic::PanicInfo;

use log::*;
use memory::active_level_4_table;
use qemu::exit_qemu;
use x86_64::VirtAddr;

use crate::{println, serial_println};

mod console;
mod gdt;
mod interrupts;
mod logging;
mod memory;
mod qemu;
mod serial;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {:#?}", info);

    loop {
        x86_64::instructions::hlt();
    }
}

pub struct X86Arch {
    pub(crate) boot_info: &'static mut bootloader_api::BootInfo,
}

impl crate::arch::Arch for X86Arch {
    fn entry(&mut self) -> ! {
        crate::arch::x86::logging::init();
        crate::arch::x86::console::init(self.boot_info.framebuffer.as_mut().unwrap());
        crate::arch::x86::interrupts::init();
        crate::arch::x86::gdt::init();
        crate::arch::x86::memory::init(self.boot_info);

        println!("Hello from the x86_64 kernel!");
        println!("More debug info will be display in the serial console.");
        serial_println!("If you can't see more content here, you need to specify LOG_LEVEL env at compile time to enable higher level log filtering.");

        info!("Hello from the x86_64 kernel!");
        info!("This is the last message from the kernel.");

        exit_qemu(0x10);

        loop {
            x86_64::instructions::hlt();
        }
    }
}
