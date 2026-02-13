use core::arch::global_asm;

global_asm!(include_str!("smp_trampoline.S"));

unsafe extern "C" {
    static ap_trampoline_start: u8;
    static ap_trampoline_end: u8;
}

pub const AP_TRAMPOLINE_DATA_OFFSET: usize = 0x800;

pub fn trampoline_bytes() -> &'static [u8] {
    let start = core::ptr::addr_of!(ap_trampoline_start) as *const u8 as usize;
    let end = core::ptr::addr_of!(ap_trampoline_end) as *const u8 as usize;
    let len = end.saturating_sub(start);
    unsafe { core::slice::from_raw_parts(start as *const u8, len) }
}

