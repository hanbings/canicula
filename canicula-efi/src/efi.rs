#![no_main]
#![no_std]

extern crate alloc;

use boot::{get_handle_for_protocol, open_protocol_exclusive};
use log::info;
use uefi::boot::{AllocateType, MemoryType};
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::File;
use uefi::proto::media::file::{FileAttribute, FileInfo, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{prelude::*, CStr16};

static KERNEL_PATH: &str = "\\kernel";
static FILE_BUFFER_SIZE: usize = 0x400;
static PAGE_SIZE: usize = 0x1000;

struct GraphicInfo {
    frame_buffer_addr: u64,
    frame_buffer_size: u64,
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("bootloader is running");

    // load simple file system protocol
    let simple_file_system_handle = get_handle_for_protocol::<SimpleFileSystem>().unwrap();
    let mut simple_file_system_protocol =
        open_protocol_exclusive::<SimpleFileSystem>(simple_file_system_handle).unwrap();

    // open volume
    let mut root = simple_file_system_protocol.open_volume().unwrap();

    // open kernel file in the root using simple file system
    let mut kernel_path_buffer = [0u16; FILE_BUFFER_SIZE];
    let kernel_path = CStr16::from_str_with_buf(KERNEL_PATH, &mut kernel_path_buffer).unwrap();
    let kernel_file_handle = root
        .open(kernel_path, FileMode::Read, FileAttribute::empty())
        .unwrap();
    let mut kernel_file = match kernel_file_handle.into_type().unwrap() {
        FileType::Regular(f) => f,
        _ => panic!("This file does not exist!"),
    };
    info!("Kernel file opened successfully!");

    // load kernel file info and size
    let mut kernel_file_info_buffer = [0u8; FILE_BUFFER_SIZE];
    let kernel_file_info: &mut FileInfo =
        kernel_file.get_info(&mut kernel_file_info_buffer).unwrap();
    info!("Kernel file info: {:?}", kernel_file_info);
    let kernel_file_size = usize::try_from(kernel_file_info.file_size()).unwrap();
    info!("Kernel file size: {:?}", kernel_file_size);

    // load kernel file into memory
    let mut kernel_file_address = uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        kernel_file_size / PAGE_SIZE + 1,
    )
    .unwrap();

    let kernel_file_address = unsafe { kernel_file_address.as_mut() as *mut u8 };

    let kernel_file_in_memory = unsafe {
        core::ptr::write_bytes(kernel_file_address, 0, kernel_file_size);
        core::slice::from_raw_parts_mut(kernel_file_address, kernel_file_size)
    };
    let kernel_file_size = kernel_file.read(kernel_file_in_memory).unwrap();
    info!("Kernel file loaded into memory successfully!");

    let kernel_content = &mut kernel_file_in_memory[..kernel_file_size];
    let kernel_address = kernel_content.as_ptr() as *const u8 as usize;
    info!("Kernel file address: 0x{:x}", kernel_address);

    // init display
    let gop_handler = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handler).unwrap();

    let graphic_info = GraphicInfo {
        frame_buffer_addr: gop.frame_buffer().as_mut_ptr() as u64,
        frame_buffer_size: gop.frame_buffer().size() as u64,
    };

    unsafe {
        core::arch::asm!(
            "
            jmp {}
            ", 
            in(reg) kernel_address,
            in("rdi") graphic_info.frame_buffer_addr,
            in("rcx") graphic_info.frame_buffer_size,
            options(noreturn)
        );
    }
}
