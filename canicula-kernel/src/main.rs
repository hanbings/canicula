#![no_std]
#![no_main]
#![allow(dead_code)]
#![cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    feature(abi_x86_interrupt)
)]
#![feature(alloc_error_handler)]

mod arch;
mod resources;
mod types;

#[unsafe(no_mangle)]
#[cfg(target_arch = "riscv64")]
pub fn kernel() -> ! {
    arch::riscv::entry();
}

#[unsafe(no_mangle)]
#[cfg(target_arch = "aarch64")]
pub fn kernel() -> ! {
    arch::aarch::entry();
}

/// Entry point called by the bootloader.
/// This naked function ensures proper stack alignment before calling the real entry.
#[unsafe(no_mangle)]
#[unsafe(naked)]
#[cfg(target_arch = "x86_64")]
pub unsafe extern "C" fn kernel_main() -> ! {
    // RDI contains the boot_info pointer (passed by bootloader)
    // Align stack to 16 bytes, then call (which pushes 8-byte return address)
    // This makes RSP % 16 == 8 at function entry, as per x86_64 ABI
    core::arch::naked_asm!(
        "and rsp, 0xFFFFFFFFFFFFFFF0", // Align stack to 16 bytes
        "call {entry}",                 // call pushes return addr, making RSP % 16 == 8
        "ud2",                          // Should never return
        entry = sym kernel_entry,
    )
}

#[cfg(target_arch = "x86_64")]
fn kernel_entry(boot_info: &'static mut canicula_common::entry::BootInfo) -> ! {
    arch::x86::entry(boot_info)
}
