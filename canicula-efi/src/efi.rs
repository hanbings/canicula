#![no_std]
#![no_main]

mod config;

use boot::{AllocateType, MemoryType};
use config::x86_64::KERNEL_STACK_SIZE;
use log::info;
use uefi::{
    prelude::*,
    proto::media::{
        file::{File, FileAttribute, FileInfo, FileMode, FileType},
        fs::SimpleFileSystem,
    },
    CStr16,
};
use xmas_elf::ElfFile;

use crate::config::x86_64::{FILE_BUFFER_SIZE, KERNEL_PATH, KERNEL_STACK_ADDRESS, PAGE_SIZE};

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("Canicula: Starting the UEFI bootloader...");
    info!(
        "
        config: file_buffer_size = {}, 
        page_size = {}, 
        kernel_path = {}, 
        kernel_stack_address = {:#X}, 
        kernel_stack_size = {:#X}
        ",
        FILE_BUFFER_SIZE, PAGE_SIZE, KERNEL_PATH, KERNEL_STACK_ADDRESS, KERNEL_STACK_SIZE
    );

    // load simple file system protocol
    let simple_file_system_handle = uefi::boot::get_handle_for_protocol::<SimpleFileSystem>()
        .expect("Cannot get protocol handle");

    let mut simple_file_system_protocol =
        uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(simple_file_system_handle)
            .expect("Cannot get simple file system protocol");

    // open volume
    let mut root = simple_file_system_protocol
        .open_volume()
        .expect("Cannot open volume");

    // open kernel file in the root using simple file system
    let mut kernel_path_buffer = [0u16; FILE_BUFFER_SIZE];
    let kernel_path = CStr16::from_str_with_buf(KERNEL_PATH, &mut kernel_path_buffer)
        .expect("Invalid kernel path!");
    let kernel_file_handle = root
        .open(kernel_path, FileMode::Read, FileAttribute::empty())
        .expect("Cannot open kernel file");
    let mut kernel_file = match kernel_file_handle.into_type().unwrap() {
        FileType::Regular(f) => f,
        _ => panic!("This file does not exist!"),
    };
    info!("Kernel file opened successfully!");

    // load kernel file info and size
    let mut kernel_file_info_buffer = [0u8; FILE_BUFFER_SIZE];
    let kernel_file_info: &mut FileInfo = kernel_file
        .get_info(&mut kernel_file_info_buffer)
        .expect("Cannot get file info");
    info!("Kernel file info: {:?}", kernel_file_info);
    let kernel_file_size =
        usize::try_from(kernel_file_info.file_size()).expect("Invalid file size!");
    info!("Kernel file size: {:?}", kernel_file_size);

    // load kernel file into memory
    let mut kernel_file_address = uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        kernel_file_size / PAGE_SIZE + 1,
    )
    .expect("Cannot allocate memory in the RAM!");

    let kernel_file_address = unsafe { kernel_file_address.as_mut() as *mut u8 };

    let kernel_file_in_memory = unsafe {
        core::ptr::write_bytes(kernel_file_address, 0, kernel_file_size);
        core::slice::from_raw_parts_mut(kernel_file_address, kernel_file_size)
    };
    let kernel_file_size = kernel_file
        .read(kernel_file_in_memory)
        .expect("Cannot read file into the memory!");
    info!("Kernel file loaded into memory successfully!");

    let kernel_content = &mut kernel_file_in_memory[..kernel_file_size];
    let kernel_address = kernel_content.as_ptr() as *const u8 as usize;
    info!("Kernel file address: 0x{:x}", kernel_address);

    // parsing kernel elf
    let kernel_elf = ElfFile::new(kernel_content).expect("Not a valid ELF file.");
    let kernel_entry_offset = kernel_elf.header.pt2.entry_point() as usize;
    let kernel_entry_address = kernel_address + kernel_entry_offset;

    // jmp to kernel
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("jmp {}", in(reg) kernel_entry_address);
    }

    uefi::boot::stall(10_000_000);
    Status::SUCCESS
}
