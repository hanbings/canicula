#![no_main]
#![no_std]

extern crate alloc;

mod canicula;
mod linux;

use uefi::prelude::*;

pub(crate) static FILE_BUFFER_SIZE: usize = 0x400;
pub(crate) static PAGE_SIZE: usize = 0x1000;

// Set BOOT_MODE to select which kernel to boot
#[derive(PartialEq)]
enum BootMode {
    /// Boot the custom Canicula kernel (ELF format)
    #[allow(dead_code)]
    CaniculaKernel,
    /// Boot a standard Linux kernel via EFI stub (PE/COFF vmlinuz)
    #[allow(dead_code)]
    LinuxEfiStub,
}

const BOOT_MODE: BootMode = BootMode::LinuxEfiStub;

// Serial port output for debugging after exit_boot_services
pub(crate) fn serial_out(c: u8) {
    let port: u16 = 0x3F8;
    loop {
        let status: u8;
        unsafe {
            core::arch::asm!(
                "in al, dx",
                out("al") status,
                in("dx") port + 5,
                options(nomem, nostack)
            );
        }
        if status & 0x20 != 0 {
            break;
        }
    }
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") c,
            options(nomem, nostack)
        );
    }
}

pub(crate) fn serial_str(s: &str) {
    for b in s.bytes() {
        serial_out(b);
    }
}

pub(crate) fn serial_hex(val: u64) {
    serial_str("0x");
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as u8;
        let c = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + nibble - 10
        };
        serial_out(c);
    }
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    log::info!("Canicula Loader starting...");

    match BOOT_MODE {
        BootMode::LinuxEfiStub => linux::boot_linux_efi_stub(),
        BootMode::CaniculaKernel => canicula::boot_canicula_kernel(),
    }
}
