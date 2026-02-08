#![no_main]
#![no_std]

extern crate alloc;

use canicula_common::entry::{
    BootInfo, FrameBuffer, FrameBufferInfo, MemoryRegion, MemoryRegionKind, MemoryRegions,
    PixelFormat,
};
use log::info;
use uefi::boot::{AllocateType, MemoryType as UefiMemoryType};
use uefi::mem::memory_map::MemoryMap;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat as UefiPixelFormat};
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{CStr16, prelude::*};
use xmas_elf::ElfFile;
use xmas_elf::program::Type;

static KERNEL_PATH: &str = "\\kernel-x86_64";
static FILE_BUFFER_SIZE: usize = 0x400;
static PAGE_SIZE: usize = 0x1000;

// Physical memory direct mapping base (identity map the first 4GB here)
const PHYSICAL_MEMORY_OFFSET: u64 = 0xffff_8800_0000_0000;

// Page table indices (for 0xfffff80000000000)
const KERNEL_PML4_INDEX: usize = 496; // (0xfffff80000000000 >> 39) & 0x1FF
const PHYS_MAP_PML4_INDEX: usize = 272; // 0xffff880000000000 >> 39 & 0x1FF

// Page table entry flags
const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_WRITABLE: u64 = 1 << 1;
const PAGE_HUGE: u64 = 1 << 7;

static mut BOOT_INFO: BootInfo = BootInfo {
    memory_regions: MemoryRegions::new(),
    framebuffer: None,
    physical_memory_offset: None,
    rsdp_addr: None,
};

/// Page table configuration for deferred initialization
struct PageTableConfig {
    pml4: u64,
    pdpt_low: u64,
    pdpt_kernel: u64,
    pdpt_phys_map: u64,
    pd_low_base: u64,
    pd_kernel: u64,
    pd_phys_map_base: u64,
    pt_base: u64,
    kernel_phys: u64,
    kernel_4k_pages: usize,
    pt_count: usize,
}

/// Allocate page-table memory (call before exit_boot_services)
unsafe fn allocate_page_tables(kernel_phys: u64, kernel_size: usize) -> PageTableConfig {
    let kernel_4k_pages = (kernel_size + PAGE_SIZE - 1) / PAGE_SIZE;
    let pt_count = (kernel_4k_pages + 511) / 512;

    // PML4 + PDPT_LOW + PDPT_KERNEL + PDPT_PHYS_MAP + PD_LOW[4] + PD_KERNEL + PD_PHYS_MAP[4] + PT[n]
    let total_pages = 1 + 3 + 4 + 1 + 4 + pt_count;
    let pages_ptr = uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        UefiMemoryType::LOADER_DATA,
        total_pages,
    )
    .expect("Failed to allocate page tables");

    let base = pages_ptr.as_ptr() as u64;
    let mut offset = 0u64;

    let pml4 = base + offset;
    offset += PAGE_SIZE as u64;

    let pdpt_low = base + offset;
    offset += PAGE_SIZE as u64;

    let pdpt_kernel = base + offset;
    offset += PAGE_SIZE as u64;

    let pdpt_phys_map = base + offset;
    offset += PAGE_SIZE as u64;

    let pd_low_base = base + offset;
    offset += 4 * PAGE_SIZE as u64;

    let pd_kernel = base + offset;
    offset += PAGE_SIZE as u64;

    let pd_phys_map_base = base + offset;
    offset += 4 * PAGE_SIZE as u64;

    let pt_base = base + offset;

    PageTableConfig {
        pml4,
        pdpt_low,
        pdpt_kernel,
        pdpt_phys_map,
        pd_low_base,
        pd_kernel,
        pd_phys_map_base,
        pt_base,
        kernel_phys,
        kernel_4k_pages,
        pt_count,
    }
}

// Serial port output for debugging after exit_boot_services
fn serial_out(c: u8) {
    let port: u16 = 0x3F8;
    loop {
        let status: u8;
        unsafe {
            core::arch::asm!(
                "in al, dx",
                out("al") status,
                in("dx") port + 5,
                options(nomem, nostack)
            );
        }
        if status & 0x20 != 0 {
            break;
        }
    }
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") c,
            options(nomem, nostack)
        );
    }
}

fn serial_str(s: &str) {
    for b in s.bytes() {
        serial_out(b);
    }
}

fn serial_hex(val: u64) {
    serial_str("0x");
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as u8;
        let c = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + nibble - 10
        };
        serial_out(c);
    }
}

/// Initialize page tables (call after exit_boot_services)
unsafe fn init_page_tables(cfg: &PageTableConfig) -> u64 {
    let pml4 = cfg.pml4 as *mut u64;
    let pdpt_low = cfg.pdpt_low as *mut u64;
    let pdpt_kernel = cfg.pdpt_kernel as *mut u64;
    let pdpt_phys_map = cfg.pdpt_phys_map as *mut u64;
    let pd_low_base = cfg.pd_low_base;
    let pd_kernel = cfg.pd_kernel as *mut u64;
    let pd_phys_map_base = cfg.pd_phys_map_base;
    let pt_base = cfg.pt_base;

    let total_pages = 1 + 3 + 4 + 1 + 4 + cfg.pt_count;

    serial_str("[PT] Initializing page tables...\r\n");

    unsafe {
        // Zero all page tables
        core::ptr::write_bytes(pml4 as *mut u8, 0, PAGE_SIZE * total_pages);

        // PML4[0] -> PDPT_LOW (identity mapping for low addresses)
        *pml4.add(0) = cfg.pdpt_low | PAGE_PRESENT | PAGE_WRITABLE;

        // PML4[KERNEL_PML4_INDEX] -> PDPT_KERNEL (kernel mapping)
        *pml4.add(KERNEL_PML4_INDEX) = cfg.pdpt_kernel | PAGE_PRESENT | PAGE_WRITABLE;

        // PML4[PHYS_MAP_PML4_INDEX] -> PDPT_PHYS_MAP (physical memory direct mapping)
        *pml4.add(PHYS_MAP_PML4_INDEX) = cfg.pdpt_phys_map | PAGE_PRESENT | PAGE_WRITABLE;

        // PDPT_LOW[0-3] -> PD_LOW[0-3] (identity map 0-4GB)
        for i in 0..4 {
            let pd_addr = pd_low_base + i as u64 * PAGE_SIZE as u64;
            *pdpt_low.add(i) = pd_addr | PAGE_PRESENT | PAGE_WRITABLE;
        }

        // PD_LOW: map the first 4GB using 2MB huge pages
        for gb in 0..4 {
            let pd = (pd_low_base + gb as u64 * PAGE_SIZE as u64) as *mut u64;
            for i in 0..512 {
                let phys_addr = (gb as u64 * 512 + i as u64) * 0x200000;
                *pd.add(i) = phys_addr | PAGE_PRESENT | PAGE_WRITABLE | PAGE_HUGE;
            }
        }

        // PDPT_PHYS_MAP[0-3] -> PD_PHYS_MAP[0-3] (physical memory direct mapping)
        for i in 0..4 {
            let pd_addr = pd_phys_map_base + i as u64 * PAGE_SIZE as u64;
            *pdpt_phys_map.add(i) = pd_addr | PAGE_PRESENT | PAGE_WRITABLE;
        }

        // PD_PHYS_MAP: map the first 4GB using 2MB huge pages
        for gb in 0..4 {
            let pd = (pd_phys_map_base + gb as u64 * PAGE_SIZE as u64) as *mut u64;
            for i in 0..512 {
                let phys_addr = (gb as u64 * 512 + i as u64) * 0x200000;
                *pd.add(i) = phys_addr | PAGE_PRESENT | PAGE_WRITABLE | PAGE_HUGE;
            }
        }

        // PDPT_KERNEL[0] -> PD_KERNEL (KERNEL_PDPT_INDEX = 0)
        *pdpt_kernel.add(0) = cfg.pd_kernel | PAGE_PRESENT | PAGE_WRITABLE;

        // PD_KERNEL -> PT
        for i in 0..cfg.pt_count {
            let pt_addr = pt_base + i as u64 * PAGE_SIZE as u64;
            *pd_kernel.add(i) = pt_addr | PAGE_PRESENT | PAGE_WRITABLE;
        }

        // PT: map each 4KB page of the kernel
        for i in 0..cfg.kernel_4k_pages {
            let pt_index = i / 512;
            let pte_index = i % 512;
            let pt = (pt_base + pt_index as u64 * PAGE_SIZE as u64) as *mut u64;
            let phys_addr = cfg.kernel_phys + i as u64 * PAGE_SIZE as u64;
            *pt.add(pte_index) = phys_addr | PAGE_PRESENT | PAGE_WRITABLE;
        }

        serial_str("[PT] Page tables initialized\r\n");
    }

    cfg.pml4
}

fn convert_memory_type(ty: UefiMemoryType) -> MemoryRegionKind {
    match ty {
        UefiMemoryType::CONVENTIONAL => MemoryRegionKind::Usable,
        UefiMemoryType::LOADER_CODE
        | UefiMemoryType::LOADER_DATA
        | UefiMemoryType::BOOT_SERVICES_CODE
        | UefiMemoryType::BOOT_SERVICES_DATA => MemoryRegionKind::Bootloader,
        _ => MemoryRegionKind::UnknownUefi(ty.0),
    }
}

fn convert_pixel_format(format: UefiPixelFormat) -> PixelFormat {
    match format {
        UefiPixelFormat::Rgb => PixelFormat::Rgb,
        UefiPixelFormat::Bgr => PixelFormat::Bgr,
        _ => PixelFormat::Unknown {
            red_position: 0,
            green_position: 8,
            blue_position: 16,
        },
    }
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("Canicula Loader starting...");

    // Load filesystem
    let simple_file_system_handle =
        uefi::boot::get_handle_for_protocol::<SimpleFileSystem>().unwrap();
    let mut simple_file_system_protocol =
        uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(simple_file_system_handle).unwrap();
    let mut root = simple_file_system_protocol.open_volume().unwrap();

    // Open kernel file
    let mut kernel_path_buffer = [0u16; FILE_BUFFER_SIZE];
    let kernel_path = CStr16::from_str_with_buf(KERNEL_PATH, &mut kernel_path_buffer).unwrap();
    let kernel_file_handle = root
        .open(kernel_path, FileMode::Read, FileAttribute::empty())
        .unwrap();
    let mut kernel_file = match kernel_file_handle.into_type().unwrap() {
        FileType::Regular(f) => f,
        _ => panic!("Kernel file does not exist!"),
    };
    info!("Kernel file opened successfully!");

    // Get kernel file size
    let mut kernel_file_info_buffer = [0u8; FILE_BUFFER_SIZE];
    let kernel_file_info: &mut FileInfo =
        kernel_file.get_info(&mut kernel_file_info_buffer).unwrap();
    let kernel_file_size = usize::try_from(kernel_file_info.file_size()).unwrap();
    info!("Kernel ELF size: {} bytes", kernel_file_size);

    // Read ELF into a temporary buffer
    let mut kernel_elf_data = alloc::vec![0u8; kernel_file_size];
    kernel_file.read(&mut kernel_elf_data).unwrap();

    // Parse ELF
    let elf = ElfFile::new(&kernel_elf_data).expect("Failed to parse ELF");
    let entry_point = elf.header.pt2.entry_point();
    info!("ELF entry point: {:#x}", entry_point);

    // Compute the virtual memory range to load
    let mut min_virt: u64 = u64::MAX;
    let mut max_virt: u64 = 0;

    for ph in elf.program_iter() {
        if ph.get_type().unwrap() == Type::Load {
            let start = ph.virtual_addr();
            let end = start + ph.mem_size();
            if start < min_virt {
                min_virt = start;
            }
            if end > max_virt {
                max_virt = end;
            }
        }
    }

    let total_size = (max_virt - min_virt) as usize;
    let num_pages = (total_size + PAGE_SIZE - 1) / PAGE_SIZE;

    info!("Kernel virtual range: {:#x} - {:#x}", min_virt, max_virt);
    info!("Kernel size: {} pages", num_pages);

    // Allocate physical memory (2MB-aligned for huge pages)
    let num_pages_aligned = ((total_size + 0x200000 - 1) / 0x200000) * 512;
    let kernel_phys_ptr = uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        UefiMemoryType::LOADER_DATA,
        num_pages_aligned,
    )
    .expect("Failed to allocate memory for kernel");

    let kernel_phys_base = kernel_phys_ptr.as_ptr() as u64;
    info!("Kernel physical base: {:#x}", kernel_phys_base);

    // Load each segment into physical memory
    for ph in elf.program_iter() {
        if ph.get_type().unwrap() == Type::Load {
            let virt_addr = ph.virtual_addr();
            let offset_from_base = virt_addr - min_virt;
            let phys_addr = kernel_phys_base + offset_from_base;

            let src_offset = ph.offset() as usize;
            let file_size = ph.file_size() as usize;
            let mem_size = ph.mem_size() as usize;

            unsafe {
                let dest = phys_addr as *mut u8;
                let src = kernel_elf_data.as_ptr().add(src_offset);
                core::ptr::copy_nonoverlapping(src, dest, file_size);

                if mem_size > file_size {
                    core::ptr::write_bytes(dest.add(file_size), 0, mem_size - file_size);
                }
            }

            info!(
                "  Loaded: virt {:#x} -> phys {:#x} ({} bytes)",
                virt_addr, phys_addr, mem_size
            );
        }
    }

    // Allocate page tables (before exit_boot_services)
    info!("Allocating page tables...");
    let pt_config = unsafe { allocate_page_tables(kernel_phys_base, total_size) };
    info!("Page table memory allocated at: {:#x}", pt_config.pml4);

    // Allocate kernel stack (1MB)
    const KERNEL_STACK_SIZE: usize = 1024 * 1024;
    let stack_pages = (KERNEL_STACK_SIZE + PAGE_SIZE - 1) / PAGE_SIZE;
    let stack_ptr = uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        UefiMemoryType::LOADER_DATA,
        stack_pages,
    )
    .expect("Failed to allocate kernel stack");
    // Stack grows downward, so stack top is at the end of allocated memory
    // Use 16-byte alignment
    let stack_top = (stack_ptr.as_ptr() as u64 + KERNEL_STACK_SIZE as u64) & !0xF;
    info!(
        "Kernel stack allocated: base={:#x}, top={:#x}",
        stack_ptr.as_ptr() as u64,
        stack_top
    );

    // Get graphics info
    let gop_handler = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handler).unwrap();

    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();
    let stride = mode_info.stride();
    let fb_addr = gop.frame_buffer().as_mut_ptr() as u64;
    let fb_size = gop.frame_buffer().size();
    let pixel_format = convert_pixel_format(mode_info.pixel_format());

    info!(
        "Screen resolution: {}x{}, stride: {}",
        width, height, stride
    );
    info!("Framebuffer address: {:#x}, size: {}", fb_addr, fb_size);

    // Get RSDP address
    let rsdp_addr = uefi::system::with_config_table(|entries| {
        for entry in entries {
            if entry.guid == uefi::table::cfg::ACPI2_GUID {
                return Some(entry.address as u64);
            }
            if entry.guid == uefi::table::cfg::ACPI_GUID {
                return Some(entry.address as u64);
            }
        }
        None
    });
    info!("RSDP address: {:?}", rsdp_addr);

    // Exit boot services
    info!("Exiting boot services...");
    let memory_map = unsafe { uefi::boot::exit_boot_services(UefiMemoryType::LOADER_DATA) };

    // Convert memory map to BootInfo format
    unsafe {
        let boot_info_ptr = core::ptr::addr_of_mut!(BOOT_INFO);

        for desc in memory_map.entries() {
            let start = desc.phys_start;
            let end = start + desc.page_count * PAGE_SIZE as u64;
            let kind = convert_memory_type(desc.ty);

            (*boot_info_ptr)
                .memory_regions
                .push(MemoryRegion { start, end, kind });
        }

        // Set framebuffer info
        (*boot_info_ptr).framebuffer = Some(FrameBuffer::new(
            fb_addr,
            fb_size,
            FrameBufferInfo {
                width,
                height,
                stride,
                bytes_per_pixel: 4,
                pixel_format,
            },
        ));

        // Set physical memory offset
        (*boot_info_ptr).physical_memory_offset = Some(PHYSICAL_MEMORY_OFFSET);

        // Set RSDP address
        (*boot_info_ptr).rsdp_addr = rsdp_addr;
    }

    // Initialize page tables after exit_boot_services
    let pml4_phys = unsafe { init_page_tables(&pt_config) };

    serial_str("[LOADER] Jumping to kernel at ");
    serial_hex(entry_point);
    serial_str("\r\n");

    // Switch page tables and jump to kernel
    unsafe {
        let boot_info_ptr = core::ptr::addr_of_mut!(BOOT_INFO);

        core::arch::asm!(
            // Set up new stack (must be 16-byte aligned for SSE)
            "mov rsp, {stack}",
            // Load new page tables
            "mov cr3, {cr3}",
            // Jump to kernel
            "jmp {entry}",
            stack = in(reg) stack_top,
            cr3 = in(reg) pml4_phys,
            entry = in(reg) entry_point,
            in("rdi") boot_info_ptr,
            options(noreturn)
        );
    }
}
