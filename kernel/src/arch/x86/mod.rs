use core::panic::PanicInfo;

pub fn entry() -> ! {
    use core::arch::asm;
    unsafe { asm!("mov rsp, 0x7fffe000; jmp _start") }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
