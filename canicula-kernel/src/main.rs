#![no_std]
#![no_main]

mod arch;

#[no_mangle]
#[cfg(target_arch = "riscv64")]
pub fn kernel() -> ! {
    arch::riscv::entry();
}

#[no_mangle]
#[cfg(target_arch = "aarch64")]
pub fn kernel() -> ! {
    arch::aarch::entry();
}

/// This is the entry point for the x86-64 (UEFI) kernel.
#[no_mangle]
#[cfg(target_arch = "x86_64")]
pub extern "C" fn kernel() -> ! {
    arch::x86::entry();
}
