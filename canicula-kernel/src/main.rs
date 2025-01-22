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

#[no_mangle]
pub extern "C" fn kernel(frame_buffer_addr: u64, frame_buffer_size: u64) -> ! {
    unsafe {
        core::arch::asm!("mov rcx, 0x12345678");
    }

    let frame_buffer_ptr = frame_buffer_addr as *mut u64;

    for i in 0..frame_buffer_size {
        unsafe {
            *frame_buffer_ptr.add(i as usize) = 0xff408deb;
        }
    }

    loop {}
}
