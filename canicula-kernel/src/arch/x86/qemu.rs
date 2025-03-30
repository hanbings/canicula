pub fn exit_qemu(exit_code: QemuExitCode) {
    unsafe {
        asm!("out dx, eax", in("dx") 0xf4, in("eax") exit_code as u32, options(nomem, nostack, preserves_flags));
    }
}