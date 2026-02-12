use core::panic::PanicInfo;

use bga::{VBE_DISPI_BPP_32, bga_set_bank, bga_set_video_mode};
use log::*;

use crate::{println, serial_println};

mod acpi;
mod memory;
mod virtualization;

mod apic;
mod bga;
mod console;
mod context;
mod gdt;
mod interrupts;
mod logging;
mod pcie;
mod process;
mod qemu;
mod scheduler;
mod serial;

extern crate alloc;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {:#?}", info);

    loop {
        x86_64::instructions::hlt();
    }
}

pub fn entry(boot_info: &'static mut canicula_common::entry::BootInfo) -> ! {
    crate::arch::x86::logging::init();
    crate::arch::x86::console::init(boot_info.framebuffer.as_mut().unwrap());

    crate::arch::x86::gdt::init();
    crate::arch::x86::interrupts::init();
    info!("GDT and IDT initialized");

    let boot_info = crate::arch::x86::memory::init(boot_info);
    info!("Memory initialized");

    crate::arch::x86::acpi::init(boot_info.rsdp_addr.as_ref().unwrap());
    info!("ACPI Initialized");

    crate::arch::x86::apic::init(boot_info.rsdp_addr.as_ref().unwrap());
    info!("APIC Initialized");

    crate::arch::x86::process::init();
    crate::arch::x86::scheduler::init();

    crate::arch::x86::pcie::init();
    info!("PCIe Initialized");

    crate::arch::x86::interrupts::enable_interrupts();
    info!("Interrupts enabled");

    println!("Hello from the x86_64 kernel!");
    println!("More debug info will be display in the serial console.");
    println!("Press Enter to poweroff.");
    serial_println!(
        "If you can't see more content here, you need to specify LOG_LEVEL env at compile time to enable higher level log filtering."
    );

    info!("Hello from the x86_64 kernel!");
    info!("This is the last message from the kernel.");

    let logo = png_decoder::decode(crate::resources::LOGO).unwrap();
    let img_width = logo.0.width;
    let img_height = logo.0.height;
    let pixels = logo.1;

    bga_set_video_mode(img_width, img_height, VBE_DISPI_BPP_32 as u32, true, true);
    bga_set_bank(0);

    // After bga_set_video_mode, the framebuffer stride equals the new width (img_width),
    // not the bootloader's original stride
    let framebuffer = boot_info.framebuffer.as_ref().unwrap().buffer().as_ptr() as *mut u32;
    unsafe {
        for y in 0..img_height {
            for x in 0..img_width {
                // source index uses image width
                let src_index = (y * img_width + x) as usize;
                // destination offset uses image width as stride (set by BGA mode)
                let dst_offset = (y as usize * img_width as usize) + x as usize;
                let pixel = pixels[src_index];
                let r = pixel[0];
                let g = pixel[1];
                let b = pixel[2];
                let a = pixel[3];
                let color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                if a > 0 {
                    *framebuffer.add(dst_offset) = color;
                } else {
                    *framebuffer.add(dst_offset) = 0x00000000;
                }
            }
        }
    }

    let vec = alloc::vec![1, 1, 4, 5, 1, 4];
    let hello = alloc::string::String::from("Hello");

    debug!("{:?}", vec);
    debug!("{:?} from the x86_64 kernel alloctor!", hello);

    crate::arch::x86::memory::alloc_test();

    loop {
        x86_64::instructions::hlt();
    }
}
