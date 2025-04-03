use core::panic::PanicInfo;

use allocator::{HEAP_SIZE, HEAP_START};
use linked_list_allocator::LockedHeap;
use log::*;
use qemu::exit_qemu;

use crate::{println, serial_println};

mod acpi;
mod allocator;
mod console;
mod gdt;
mod interrupts;
mod logging;
mod memory;
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

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    error!("allocation error: {:?}", layout);

    loop {
        x86_64::instructions::hlt();
    }
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn entry(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    crate::arch::x86::logging::init();
    crate::arch::x86::console::init(boot_info.framebuffer.as_mut().unwrap());
    crate::arch::x86::interrupts::init();
    crate::arch::x86::gdt::init();
    let (mut mapper, mut frame_allocator, boot_info) = crate::arch::x86::memory::init(boot_info);
    let _ = crate::arch::x86::allocator::init(&mut mapper, &mut frame_allocator);
    let (_rsdp, _rsdt) = crate::arch::x86::acpi::init(boot_info, &mut mapper, &mut frame_allocator);

    unsafe {
        let heap_start = HEAP_START as *mut u8;
        let heap_size = HEAP_SIZE;

        ALLOCATOR.lock().init(heap_start, heap_size);
    }

    println!("Hello from the x86_64 kernel!");
    println!("More debug info will be display in the serial console.");
    serial_println!("If you can't see more content here, you need to specify LOG_LEVEL env at compile time to enable higher level log filtering.");

    info!("Hello from the x86_64 kernel!");
    info!("This is the last message from the kernel.");

    let vec = alloc::vec![1, 1, 4, 5, 1, 4];
    let hello = alloc::string::String::from("Hello");

    warn!("{:?}", vec);
    warn!("{:?} from the x86_64 kernel!", hello);

    exit_qemu(0x10);

    loop {
        x86_64::instructions::hlt();
    }
}
