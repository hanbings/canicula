#![no_std]
#![no_main]
#![deny(warnings)]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;
extern crate rlibc;

use alloc::boxed::Box;
use canicula_efi::{BootInfo, GraphicInfo, MemoryMap};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::*;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::*;
use uefi::table::cfg::{ACPI2_GUID, SMBIOS_GUID};
use x86_64::align_up;
use x86_64::registers::control::*;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::mapper::UnmapError;
use x86_64::structures::paging::*;
use x86_64::{PhysAddr, VirtAddr};
use xmas_elf::ElfFile;

mod config;

const CONFIG_PATH: &str = "\\EFI\\BOOT\\rboot.conf";

#[entry]
fn efi_main(image: uefi::Handle, mut st: SystemTable<Boot>) -> Status {
    // Initialize utilities (logging, memory allocation...)
    // uefi_services::init(&mut st).expect("failed to initialize utilities");
    uefi::helpers::init(&mut st).unwrap();

    info!("bootloader is running");
    let bs = st.boot_services();
    let config = {
        let mut file = open_file(bs, CONFIG_PATH);
        let buf = load_file(bs, &mut file);
        config::Config::parse(buf)
    };

    let acpi2_addr = st
        .config_table()
        .iter()
        .find(|entry| entry.guid == ACPI2_GUID)
        .expect("failed to find ACPI 2 RSDP")
        .address;
    info!("acpi2: {:?}", acpi2_addr);

    let smbios_addr = st
        .config_table()
        .iter()
        .find(|entry| entry.guid == SMBIOS_GUID)
        .expect("failed to find SMBIOS")
        .address;
    info!("smbios: {:?}", smbios_addr);

    let elf = {
        let mut file = open_file(bs, config.kernel_path);
        let buf = load_file(bs, &mut file);
        ElfFile::new(buf).expect("failed to parse ELF")
    };
    unsafe {
        ENTRY = elf.header.pt2.entry_point() as usize;
    }

    let max_mmap_size = st.boot_services().memory_map_size();
    let mmap_storage = Box::leak(
        vec![0; max_mmap_size.map_size + 10 * max_mmap_size.entry_size].into_boxed_slice(),
    );
    let binding = st
        .boot_services()
        .memory_map(mmap_storage)
        .expect("failed to get memory map");
    let mmap_iter = binding.entries();

    let max_phys_addr = mmap_iter
        .map(|m| m.phys_start + m.page_count * 0x1000)
        .max()
        .unwrap()
        .max(0x1_0000_0000); // include IOAPIC MMIO area

    let mut page_table = current_page_table();
    // root page table is readonly
    // disable write protect
    unsafe {
        Cr0::update(|f| f.remove(Cr0Flags::WRITE_PROTECT));
        Efer::update(|f| f.insert(EferFlags::NO_EXECUTE_ENABLE));
    }
    map_elf(&elf, &mut page_table, &mut UEFIFrameAllocator(bs)).expect("failed to map ELF");
    map_stack(
        config.kernel_stack_address,
        config.kernel_stack_size,
        &mut page_table,
        &mut UEFIFrameAllocator(bs),
    )
    .expect("failed to map stack");
    map_physical_memory(
        config.physical_memory_offset,
        max_phys_addr,
        &mut page_table,
        &mut UEFIFrameAllocator(bs),
    );
    // recover write protect
    unsafe {
        Cr0::update(|f| f.insert(Cr0Flags::WRITE_PROTECT));
    }

    let binding = st
        .boot_services()
        .memory_map(mmap_storage)
        .expect("failed to get memory map");
    let mmap_iter = binding.entries();

    let iter = mmap_iter.cloned().collect();

    info!("config: {:#x?}", config);
    let graphic_info = init_graphic(bs, config.resolution);

    info!("exit boot services");
    let (rt, _mmap_iter) = st.exit_boot_services(MemoryType::custom(0x80000000));

    // construct BootInfo
    let bootinfo = BootInfo {
        memory_map: MemoryMap { iter },
        physical_memory_offset: config.physical_memory_offset,
        graphic_info,
        system_table: rt,
    };
    let stacktop = config.kernel_stack_address + config.kernel_stack_size * 0x1000;
    unsafe {
        jump_to_entry(&bootinfo, stacktop);
    }
}

/// Open file at `path`
fn open_file(bs: &BootServices, path: &str) -> RegularFile {
    let simple_file_system_handle = bs
        .get_handle_for_protocol::<SimpleFileSystem>()
        .expect("Cannot get protocol handle");

    let mut fs = bs
        .open_protocol_exclusive::<SimpleFileSystem>(simple_file_system_handle)
        .expect("Cannot get simple file system protocol");

    info!("opening file: {}", path);

    let mut root = fs.open_volume().expect("failed to open volume");
    let mut buf = [0; 100];
    let handle = root
        .open(
            &uefi::CStr16::from_str_with_buf(path, &mut buf)
                .expect("failed to convert path to CStr16"),
            FileMode::Read,
            FileAttribute::empty(),
        )
        .expect("failed to open file");

    match handle.into_type().expect("failed to into_type") {
        FileType::Regular(regular) => regular,
        _ => panic!("Invalid file type"),
    }
}

/// Load file to new allocated pages
fn load_file(bs: &BootServices, file: &mut RegularFile) -> &'static mut [u8] {
    info!("loading file to memory");
    let mut info_buf = [0u8; 0x100];
    let info = file
        .get_info::<FileInfo>(&mut info_buf)
        .expect("failed to get file info");
    let pages = info.file_size() as usize / 0x1000 + 1;
    let mem_start = bs
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages)
        .expect("failed to allocate pages");
    let buf = unsafe { core::slice::from_raw_parts_mut(mem_start as *mut u8, pages * 0x1000) };
    let len = file.read(buf).expect("failed to read file");
    info!("file size={}", len);
    &mut buf[..len]
}

/// If `resolution` is some, then set graphic mode matching the resolution.
/// Return information of the final graphic mode.
fn init_graphic(bs: &BootServices, resolution: Option<(usize, usize)>) -> GraphicInfo {
    let graphics_output_protocol_handle = bs.get_handle_for_protocol::<GraphicsOutput>().unwrap();

    let mut gop = bs
        .open_protocol_exclusive::<GraphicsOutput>(graphics_output_protocol_handle)
        .unwrap();

    if let Some(resolution) = resolution {
        let _mode = gop
            .modes(&bs)
            .map(|mode| {
                info!("mode = {:?}", mode.info());
                mode
            })
            .find(|ref mode| {
                let info = mode.info();
                info.resolution() == resolution
            })
            .expect("graphic mode not found");
        info!("switching graphic mode");
        // gop.set_mode(&mode).expect("Failed to set graphics mode");
    }

    GraphicInfo {
        mode: gop.current_mode_info(),
        fb_addr: gop.frame_buffer().as_mut_ptr() as u64,
        fb_size: gop.frame_buffer().size() as u64,
    }
}

/// Get current page table from CR3
fn current_page_table() -> OffsetPageTable<'static> {
    let p4_table_addr = Cr3::read().0.start_address().as_u64();
    let p4_table = unsafe { &mut *(p4_table_addr as *mut PageTable) };
    unsafe { OffsetPageTable::new(p4_table, VirtAddr::new(0)) }
}

/// Use `BootServices::allocate_pages()` as frame allocator
struct UEFIFrameAllocator<'a>(&'a BootServices);

unsafe impl FrameAllocator<Size4KiB> for UEFIFrameAllocator<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let addr = self
            .0
            .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
            .expect("failed to allocate frame");
        let frame = PhysFrame::containing_address(PhysAddr::new(addr));
        Some(frame)
    }
}


/// Jump to ELF entry according to global variable `ENTRY`
unsafe fn jump_to_entry(bootinfo: *const BootInfo, stacktop: u64) -> ! {
    core::arch::asm!("mov rsp, {1}; call {}", in(reg) ENTRY, in(reg) stacktop, in("rdi") bootinfo);
    loop {}
}

/// The entry point of kernel, set by BSP.
static mut ENTRY: usize = 0;

/// 加载 ELF 文件
///
/// 遍历 ELF 的每个段，然后将代码加载到新的帧，并设置当前的页表
/// 不对 ELF 文件的加载地址做出假设
pub fn map_elf(
    elf: &ElfFile,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    debug!("mapping ELF");
    let kernel_start = PhysAddr::new(elf.input.as_ptr() as u64);
    for segment in elf.program_iter() {
        map_segment(&segment, kernel_start, page_table, frame_allocator)?;
    }
    Ok(())
}

/// 卸载 ELF 文件
pub fn unmap_elf(elf: &ElfFile, page_table: &mut impl Mapper<Size4KiB>) -> Result<(), UnmapError> {
    debug!("unmapping ELF");
    let kernel_start = PhysAddr::new(elf.input.as_ptr() as u64);
    for segment in elf.program_iter() {
        unmap_segment(&segment, kernel_start, page_table)?;
    }
    Ok(())
}

/// 加载 ELF 文件栈
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

use xmas_elf::program;

fn map_segment(
    segment: &program::ProgramHeader,
    kernel_start: PhysAddr,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    if segment.get_type().unwrap() != program::Type::Load {
        return Ok(());
    }
    debug!("mapping segment: {:#x?}", segment);
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

fn unmap_segment(
    segment: &program::ProgramHeader,
    kernel_start: PhysAddr,
    page_table: &mut impl Mapper<Size4KiB>,
) -> Result<(), UnmapError> {
    if segment.get_type().unwrap() != program::Type::Load {
        return Ok(());
    }
    debug!("unmapping segment: {:#x?}", segment);
    let mem_size = segment.mem_size();
    let file_size = segment.file_size();
    let file_offset = segment.offset() & !0xfff;
    let phys_start_addr = kernel_start + file_offset;
    let virt_start_addr = VirtAddr::new(segment.virtual_addr());

    let start_page: Page = Page::containing_address(virt_start_addr);
    let start_frame = PhysFrame::<Size4KiB>::containing_address(phys_start_addr);
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
        page_table.unmap(page)?.1.flush();
    }

    if mem_size > file_size {
        // .bss section (or similar), which needs to be zeroed
        let zero_start = virt_start_addr + file_size;
        let zero_end = virt_start_addr + mem_size;
        if zero_start.as_u64() & 0xfff != 0 {
            // A part of the last mapped frame needs to be zeroed. This is
            // not possible since it could already contains parts of the next
            // segment. Thus, we need to copy it before zeroing.

            let last_page = Page::containing_address(virt_start_addr + file_size - 1u64);

            page_table.unmap(last_page)?.1.flush();
        }

        // Map additional frames.
        let start_page: Page =
            Page::containing_address(VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE)));
        let end_page = Page::containing_address(zero_end);
        for page in Page::range_inclusive(start_page, end_page) {
            page_table.unmap(page)?.1.flush();
        }
    }
    Ok(())
}

/// Map physical memory [0, max_addr)
/// to virtual space [offset, offset + max_addr)
pub fn map_physical_memory(
    offset: u64,
    max_addr: u64,
    page_table: &mut impl Mapper<Size2MiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    debug!("mapping physical memory");
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
