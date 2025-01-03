#![no_main]
#![no_std]

extern crate alloc;

use log::{debug, info};
use uefi::boot::{AllocateType, MemoryType};
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::File;
use uefi::proto::media::file::{FileAttribute, FileInfo, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{prelude::*, CStr16};
use x86_64::registers::control::{Cr0, Cr0Flags, Cr3, Efer, EferFlags};
use x86_64::structures::paging::mapper::{MapToError, UnmapError};
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags, PhysFrame,
    Size2MiB, Size4KiB,
};
use x86_64::{align_up, PhysAddr, VirtAddr};
use xmas_elf::{program, ElfFile};

static KERNEL_PATH: &str = "\\canicula-kernel";
static KERNEL_STACK_ADDRESS: u64 = 0xFFFF_FF01_0000_0000;
static KERNEL_STACK_SIZE: u64 = 512;
static PHYSICAL_MEMORY_OFFSET: u64 = 0xFFFF_8000_0000_0000;
static FILE_BUFFER_SIZE: usize = 0x400;
static PAGE_SIZE: usize = 0x1000;

struct UEFIFrameAllocator();

unsafe impl FrameAllocator<Size4KiB> for UEFIFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let address =
            boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1).unwrap();

        info!("allocate frame: {:#x?}", address);

        Some(PhysFrame::containing_address(PhysAddr::new(
            address.as_ptr() as u64,
        )))
    }
}

struct GraphicInfo {
    frame_buffer_addr: u64,
    frame_buffer_size: u64,
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("bootloader is running");

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
    let kernel_entry_point = kernel_elf.header.pt2.entry_point() as usize;

    info!("elf file: {:?}", kernel_entry_point);

    // create a new page table
    info!("create a new page table");
    let mut page_table = {
        let p4_table_address = Cr3::read().0.start_address().as_u64();
        let p4_table = unsafe { &mut *(p4_table_address as *mut PageTable) };

        unsafe { OffsetPageTable::new(p4_table, VirtAddr::new(0)) }
    };

    info!("Stalling for 5 seconds...");
    boot::stall(5_000_000);

    // root page table is readonly
    // disable write protect
    info!("disable write protect");
    unsafe {
        Cr0::update(|f| f.remove(Cr0Flags::WRITE_PROTECT));
        Efer::update(|f| f.insert(EferFlags::NO_EXECUTE_ENABLE));
    }

    // mapping the kernel
    info!("mapping the kernel");
    {
        let kernel_start = PhysAddr::new(kernel_elf.input.as_ptr() as u64);
        for segment in kernel_elf.program_iter() {
            map_segment(
                &segment,
                kernel_start,
                &mut page_table,
                &mut UEFIFrameAllocator(),
            )
            .expect("failed to map segment");
        }
    }

    {
        map_stack(
            KERNEL_STACK_ADDRESS,
            KERNEL_STACK_SIZE,
            &mut page_table,
            &mut UEFIFrameAllocator(),
        )
        .expect("failed to map stack");
    }

    {
        map_physical_memory(
            PHYSICAL_MEMORY_OFFSET,
            0x1_0000_0000,
            &mut page_table,
            &mut UEFIFrameAllocator(),
        );
    }

    // enable write protect
    info!("enable write protect");
    unsafe {
        Cr0::update(|f| f.insert(Cr0Flags::WRITE_PROTECT));
    }

    // init display
    let gop_handler = uefi::boot::get_handle_for_protocol::<GraphicsOutput>()
        .expect("failed to get GraphicsOutput");
    let mut gop = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handler)
        .expect("failed to open GraphicsOutput");

    let graphic_info = GraphicInfo {
        frame_buffer_addr: gop.frame_buffer().as_mut_ptr() as u64,
        frame_buffer_size: gop.frame_buffer().size() as u64,
    };

    // exit boot services
    info!("exit boot services");
    let _memory_map;
    unsafe {
        _memory_map = uefi::boot::exit_boot_services(MemoryType::BOOT_SERVICES_DATA);
    }

    unsafe {
        core::arch::asm!("mov rsp, {stack}", stack = in(reg) KERNEL_STACK_ADDRESS);
        core::arch::asm!("mov rbp, rsp");
        core::arch::asm!("mov rdi, {graphic_info}", graphic_info = in(reg) &graphic_info);
        core::arch::asm!("jmp {kernel}", kernel = in(reg) kernel_entry_point, options(noreturn));
    }
}

pub fn map_stack(
    addr: u64,
    pages: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    debug!("mapping stack at {:#x}", addr);
    // create a stack
    let stack_start = Page::containing_address(VirtAddr::new(addr));
    let stack_end = stack_start + pages;

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    for page in Page::range(stack_start, stack_end) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)?
                .flush();
        }
    }

    Ok(())
}

fn map_segment(
    segment: &program::ProgramHeader,
    kernel_start: PhysAddr,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    if segment.get_type().unwrap() != program::Type::Load {
        return Ok(());
    }
    info!("mapping segment: {:#x?}", segment);
    let mem_size = segment.mem_size();
    let file_size = segment.file_size();
    let file_offset = segment.offset() & !0xfff;
    let phys_start_addr = kernel_start + file_offset;
    let virt_start_addr = VirtAddr::new(segment.virtual_addr());

    let start_page: Page = Page::containing_address(virt_start_addr);
    let start_frame = PhysFrame::containing_address(phys_start_addr);
    let end_frame = PhysFrame::containing_address(phys_start_addr + file_size - 1u64);

    let flags = segment.flags();
    let mut page_table_flags = PageTableFlags::PRESENT;
    if !flags.is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE
    };
    if flags.is_write() {
        page_table_flags |= PageTableFlags::WRITABLE
    };

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let offset = frame - start_frame;
        let page = start_page + offset;
        unsafe {
            page_table
                .map_to(page, frame, page_table_flags, frame_allocator)?
                .flush();
        }
    }

    if mem_size > file_size {
        // .bss section (or similar), which needs to be zeroed
        let zero_start = virt_start_addr + file_size;
        let zero_end = virt_start_addr + mem_size;
        if zero_start.as_u64() & 0xfff != 0 {
            // A part of the last mapped frame needs to be zeroed. This is
            // not possible since it could already contains parts of the next
            // segment. Thus, we need to copy it before zeroing.

            let new_frame = frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;

            type PageArray = [u64; Size4KiB::SIZE as usize / 8];

            let last_page = Page::containing_address(virt_start_addr + file_size - 1u64);
            let last_page_ptr = end_frame.start_address().as_u64() as *mut PageArray;
            let temp_page_ptr = new_frame.start_address().as_u64() as *mut PageArray;

            unsafe {
                // copy contents
                temp_page_ptr.write(last_page_ptr.read());
            }

            // remap last page
            if let Err(e) = page_table.unmap(last_page) {
                return Err(match e {
                    UnmapError::ParentEntryHugePage => MapToError::ParentEntryHugePage,
                    UnmapError::PageNotMapped => unreachable!(),
                    UnmapError::InvalidFrameAddress(_) => unreachable!(),
                });
            }

            unsafe {
                page_table
                    .map_to(last_page, new_frame, page_table_flags, frame_allocator)?
                    .flush();
            }
        }

        // Map additional frames.
        let start_page: Page =
            Page::containing_address(VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE)));
        let end_page = Page::containing_address(zero_end);
        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;
            unsafe {
                page_table
                    .map_to(page, frame, page_table_flags, frame_allocator)?
                    .flush();
            }
        }

        // zero bss
        unsafe {
            core::ptr::write_bytes(
                zero_start.as_mut_ptr::<u8>(),
                0,
                (mem_size - file_size) as usize,
            );
        }
    }
    Ok(())
}

pub fn map_physical_memory(
    offset: u64,
    max_addr: u64,
    page_table: &mut impl Mapper<Size2MiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    info!("mapping physical memory");
    let start_frame = PhysFrame::containing_address(PhysAddr::new(0));
    let end_frame = PhysFrame::containing_address(PhysAddr::new(max_addr));
    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64() + offset));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)
                .expect("failed to map physical memory")
                .flush();
        }
    }
}
