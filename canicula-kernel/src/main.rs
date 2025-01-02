#![no_std]
#![no_main]

use core::arch::asm;

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

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {
        hlt();
    }
}

/// hlt 指令的封装
#[inline(always)]
fn hlt() {
    unsafe {
        asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}
