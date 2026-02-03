use core::arch::global_asm;
use log::*;
use qemu::QEMU_EXIT_HANDLE;
use qemu::QEMUExit;

use crate::println;

#[macro_use]
mod panic;
mod console;
mod logging;
mod qemu;
mod sbi;

pub fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as *const () as usize..ebss as *const () as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}

global_asm!(include_str!("entry.asm"));

pub fn entry() -> ! {
    unsafe extern "C" {
        // begin addr of text segment
        fn stext();
        fn etext();
        // start addr of Read-Only data segment
        fn srodata();
        fn erodata();
        // start addr of data segment
        fn sdata();
        fn edata();
        // start addr of BSS segment
        fn sbss();
        fn ebss();
        // stack lower bound
        fn boot_stack_lower_bound();
        fn boot_stack_top();
    }
    clear_bss();
    logging::init();
    println!("[kernel] Hello, world!");
    debug!(
        "[kernel] .text [{:#x}, {:#x})",
        stext as *const () as usize, etext as *const () as usize
    );
    debug!(
        "[kernel] .rodata [{:#x}, {:#x})",
        srodata as *const () as usize, erodata as *const () as usize
    );
    debug!(
        "[kernel] .data [{:#x}, {:#x})",
        sdata as *const () as usize, edata as *const () as usize
    );
    debug!(
        "[kernel] boot_stack top=bottom={:#x}, lower_bound={:#x}",
        boot_stack_top as *const () as usize, boot_stack_lower_bound as *const () as usize
    );
    debug!("[kernel] .bss [{:#x}, {:#x})", sbss as *const () as usize, ebss as *const () as usize);

    QEMU_EXIT_HANDLE.exit_success();
}
