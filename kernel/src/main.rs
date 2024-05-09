#![no_std]
#![no_main]
#![feature(panic_info_message)]
use crate::qemu::QEMUExit;
use crate::qemu::QEMU_EXIT_HANDLE;
use core::arch::global_asm;
use log::*;

#[macro_use]
mod panic;
mod console;
mod logging;
mod qemu;
mod sbi;

global_asm!(include_str!("entry.asm"));

pub fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}

#[no_mangle]
pub fn rust_main() -> ! {
    extern "C" {
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
        stext as usize, etext as usize
    );
    debug!(
        "[kernel] .rodata [{:#x}, {:#x})",
        srodata as usize, erodata as usize
    );
    debug!(
        "[kernel] .data [{:#x}, {:#x})",
        sdata as usize, edata as usize
    );
    debug!(
        "[kernel] boot_stack top=bottom={:#x}, lower_bound={:#x}",
        boot_stack_top as usize, boot_stack_lower_bound as usize
    );
    debug!("[kernel] .bss [{:#x}, {:#x})", sbss as usize, ebss as usize);

    QEMU_EXIT_HANDLE.exit_success();
}
