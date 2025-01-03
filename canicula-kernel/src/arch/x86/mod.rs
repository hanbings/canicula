use core::{arch::asm, panic::PanicInfo};

pub fn entry() -> ! {
    loop {
        hlt();
    }
}

#[inline(always)]
fn hlt() {
    unsafe {
        asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
