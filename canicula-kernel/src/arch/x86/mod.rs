use core::panic::PanicInfo;

use log::*;
use memory::virtual_to_physical;
use qemu::exit_qemu;
use x86_64::{
    structures::paging::{Page, Size4KiB},
    VirtAddr,
};

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

        println!("Hello from the x86_64 kernel!");
        println!("More debug info will be display in the serial console.");
        serial_println!("If you can't see more content here, you need to specify LOG_LEVEL env at compile time to enable higher level log filtering.");

        info!("Hello from the x86_64 kernel!");
        info!("This is the last message from the kernel.");

        let mut mapper = crate::arch::x86::memory::init(self.boot_info);
        let mut frame_allocator = crate::arch::x86::memory::AbyssFrameAllocator;
        let page = Page::containing_address(VirtAddr::new(0x114514).align_up(8 as u64));
        crate::arch::x86::memory::create_example_mapping(
            0,
            page,
            &mut mapper,
            &mut frame_allocator,
        );

        let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
        unsafe {
            page_ptr.write_volatile(0x114514);
        };

        warn!("Read from Origin Virt {:x}", unsafe {
            page_ptr.read_volatile()
        });

        exit_qemu(0x10);

        loop {
            x86_64::instructions::hlt();
        }
    }
}
