use core::panic::PanicInfo;

use log::*;

use crate::{println, serial_println};

mod acpi;
mod allocator;
mod apic;
mod console;
mod gdt;
mod interrupts;
mod logging;
mod memory;
mod pcie;
mod process;
mod qemu;
mod serial;

extern crate alloc;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {:#?}", info);

    loop {
        x86_64::instructions::hlt();
    }
}

pub fn entry(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    crate::arch::x86::logging::init();
    crate::arch::x86::console::init(boot_info.framebuffer.as_mut().unwrap());

    crate::arch::x86::interrupts::init();
    crate::arch::x86::gdt::init();
    info!("Interrupts initialized");

    let (mut mapper, mut frame_allocator, boot_info) = crate::arch::x86::memory::init(boot_info);
    let _ = crate::arch::x86::allocator::init(&mut mapper, &mut frame_allocator);
    info!("Memory initialized");

    crate::arch::x86::acpi::init(boot_info.rsdp_addr.as_ref().unwrap());
    info!("ACPI Initialized");

    crate::arch::x86::apic::init(boot_info.rsdp_addr.as_ref().unwrap());
    crate::arch::x86::interrupts::enable_interrupts();
    info!("APIC Initialized");

    crate::arch::x86::pcie::init();
    info!("PCIe Initialized");

    println!("Hello from the x86_64 kernel!");
    println!("More debug info will be display in the serial console.");
    println!("Press Enter to poweroff.");
    serial_println!("If you can't see more content here, you need to specify LOG_LEVEL env at compile time to enable higher level log filtering.");

    info!("Hello from the x86_64 kernel!");
    info!("This is the last message from the kernel.");

    let vec = alloc::vec![1, 1, 4, 5, 1, 4];
    let hello = alloc::string::String::from("Hello");

    debug!("{:?}", vec);
    debug!("{:?} from the x86_64 kernel!", hello);

    loop {
        x86_64::instructions::hlt();
    }
}
