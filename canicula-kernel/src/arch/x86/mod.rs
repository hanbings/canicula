use core::panic::PanicInfo;

use bga::{bga_set_bank, bga_set_video_mode, VBE_DISPI_BPP_32};
use log::*;

use crate::{println, serial_println};

mod acpi;
mod apic;
mod bga;
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

static LOGO: &'static [u8] = include_bytes!("../../../../resources/images/logo.png");

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

    let boot_info = crate::arch::x86::memory::init(boot_info);
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

    let logo = png_decoder::decode(LOGO).unwrap();
    let width = logo.0.width;
    let height = logo.0.height;
    let pixels = logo.1;

    bga_set_video_mode(width, height, VBE_DISPI_BPP_32 as u32, true, true);
    bga_set_bank(0);

    let framebuffer = boot_info.framebuffer.as_ref().unwrap().buffer().as_ptr() as *mut u32;
    unsafe {
        for y in 0..height {
            for x in 0..width {
                let index = y * width + x;
                let r = pixels[(index * 4) as usize];
                let g = pixels[(index * 4 + 1) as usize];
                let b = pixels[(index * 4 + 2) as usize];
                let a = pixels[(index * 4 + 3) as usize];
                let color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                if a > 0 {
                    *framebuffer.offset(index as isize) = color;
                } else {
                    *framebuffer.offset(index as isize) = 0x00000000;
                }
            }
        }
    }

    let big: alloc::vec::Vec<i32> = alloc::vec::Vec::with_capacity(1024);
    let vec = alloc::vec![1, 1, 4, 5, 1, 4];
    let hello = alloc::string::String::from("Hello");

    debug!("{:?}", big.len());
    debug!("{:?}", vec);
    debug!("{:?} from the x86_64 kernel alloctor!", hello);

    crate::arch::x86::memory::alloc_test();

    loop {
        x86_64::instructions::hlt();
    }
}
