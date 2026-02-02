#![no_std]
#![no_main]

use core::arch::asm;
use core::fmt::{self, Write};
use core::panic::PanicInfo;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MemoryType {
    Reserved = 0,
    LoaderCode = 1,
    LoaderData = 2,
    BootServicesCode = 3,
    BootServicesData = 4,
    RuntimeServicesCode = 5,
    RuntimeServicesData = 6,
    Conventional = 7,
    Unusable = 8,
    ACPIReclaimable = 9,
    ACPIMemoryNVS = 10,
    MemoryMappedIO = 11,
    MemoryMappedIOPortSpace = 12,
    PalCode = 13,
    PersistentMemory = 14,
    Unknown = 0xFFFF,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryDescriptor {
    pub memory_type: MemoryType,
    pub physical_start: u64,
    pub virtual_start: u64,
    pub page_count: u64,
    pub attribute: u64,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FrameBufferInfo {
    pub addr: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub _padding: u32,
}

#[repr(C)]
pub struct BootInfo {
    pub framebuffer: FrameBufferInfo,
    pub memory_map_count: u64,
    pub memory_map: *const MemoryDescriptor,
}

const COM1: u16 = 0x3F8;

#[inline(always)]
fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
fn inb(port: u16) -> u8 {
    let ret: u8;
    unsafe {
        asm!("in al, dx", out("al") ret, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    ret
}

#[inline(always)]
fn outl(port: u16, val: u32) {
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack, preserves_flags));
    }
}

/// QEMU exit code: actual exit status = (exit_code << 1) | 1
/// Example: exit_code=0 -> QEMU exit status 1, exit_code=1 -> QEMU exit status 3
pub fn qemu_shutdown(exit_code: u32) -> ! {
    outl(0xf4, exit_code);
    // If shutdown fails, enter the halt loop
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

fn serial_init() {
    outb(COM1 + 1, 0x00); // Disable all interrupts
    outb(COM1 + 3, 0x80); // Enable DLAB
    outb(COM1 + 0, 0x03); // Baud rate 38400 (low byte)
    outb(COM1 + 1, 0x00); // Baud rate (high byte)
    outb(COM1 + 3, 0x03); // 8N1
    outb(COM1 + 2, 0xC7); // Enable FIFO
    outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
}

fn serial_write_byte(byte: u8) {
    while inb(COM1 + 5) & 0x20 == 0 {
        core::hint::spin_loop();
    }
    outb(COM1, byte);
}

struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                serial_write_byte(b'\r');
            }
            serial_write_byte(byte);
        }
        Ok(())
    }
}

static WRITER: Mutex<SerialWriter> = Mutex::new(SerialWriter);

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    // Avoid `static mut` (Rust 2024 compatibility) and serialize concurrent output.
    WRITER.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    serial_init();

    if boot_info.is_null() {
        println!("[ERROR] BootInfo pointer is null!");
        halt_loop();
    }

    unsafe {
        let info = &*boot_info;

        println!("[INFO] Framebuffer:");
        println!("  Address: {:#x}", info.framebuffer.addr);
        println!(
            "  Resolution: {}x{}",
            info.framebuffer.width, info.framebuffer.height
        );
        println!("  Stride: {}", info.framebuffer.stride);
        println!();

        println!("[INFO] Memory Map ({} entries):", info.memory_map_count);

        let mut total_memory: u64 = 0;
        let mut usable_memory: u64 = 0;

        for i in 0..info.memory_map_count {
            let desc = &*info.memory_map.add(i as usize);
            let size_kb = desc.page_count * 4;
            let size_mb = size_kb / 1024;

            total_memory += size_kb;
            if desc.memory_type == MemoryType::Conventional {
                usable_memory += size_kb;
                if size_mb >= 1 {
                    println!(
                        "  [Conventional] {:#x} - {} MB",
                        desc.physical_start, size_mb
                    );
                }
            }
        }

        println!();
        println!("  Total Memory:  {} MB", total_memory / 1024);
        println!("  Usable Memory: {} MB", usable_memory / 1024);
        println!();

        println!("[INFO] Kernel initialization complete.");
        println!("[INFO] Shutting down QEMU...");
    }

    qemu_shutdown(0); // Exit successfully
}

fn halt_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("  KERNEL PANIC!");
    if let Some(location) = info.location() {
        println!(
            "  at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }
    println!("  {}", info.message());
    halt_loop();
}
