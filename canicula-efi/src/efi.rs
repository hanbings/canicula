#![no_main]
#![no_std]

extern crate alloc;

use boot::{get_handle_for_protocol, open_protocol_exclusive};
use log::info;
use uefi::boot::{AllocateType, MemoryType as UefiMemoryType};
use uefi::mem::memory_map::MemoryMap;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::File;
use uefi::proto::media::file::{FileAttribute, FileInfo, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{CStr16, prelude::*};
use xmas_elf::ElfFile;
use xmas_elf::program::Type;

static KERNEL_PATH: &str = "\\kernel";
static FILE_BUFFER_SIZE: usize = 0x400;
static PAGE_SIZE: usize = 0x1000;

// Higher-half kernel virtual base address
const KERNEL_VIRT_BASE: u64 = 0xfffff80000000000;

// Page table indices (computed from KERNEL_VIRT_BASE)
// PML4 index: (0xFFFFF80000000000 >> 39) & 0x1FF = 496
// PDPT index: (0xFFFFF80000000000 >> 30) & 0x1FF = 0
const KERNEL_PML4_INDEX: usize = 496;
const KERNEL_PDPT_INDEX: usize = 0;

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

impl From<UefiMemoryType> for MemoryType {
    fn from(t: UefiMemoryType) -> Self {
        match t {
            UefiMemoryType::RESERVED => MemoryType::Reserved,
            UefiMemoryType::LOADER_CODE => MemoryType::LoaderCode,
            UefiMemoryType::LOADER_DATA => MemoryType::LoaderData,
            UefiMemoryType::BOOT_SERVICES_CODE => MemoryType::BootServicesCode,
            UefiMemoryType::BOOT_SERVICES_DATA => MemoryType::BootServicesData,
            UefiMemoryType::RUNTIME_SERVICES_CODE => MemoryType::RuntimeServicesCode,
            UefiMemoryType::RUNTIME_SERVICES_DATA => MemoryType::RuntimeServicesData,
            UefiMemoryType::CONVENTIONAL => MemoryType::Conventional,
            UefiMemoryType::UNUSABLE => MemoryType::Unusable,
            UefiMemoryType::ACPI_RECLAIM => MemoryType::ACPIReclaimable,
            UefiMemoryType::ACPI_NON_VOLATILE => MemoryType::ACPIMemoryNVS,
            UefiMemoryType::MMIO => MemoryType::MemoryMappedIO,
            UefiMemoryType::MMIO_PORT_SPACE => MemoryType::MemoryMappedIOPortSpace,
            UefiMemoryType::PAL_CODE => MemoryType::PalCode,
            UefiMemoryType::PERSISTENT_MEMORY => MemoryType::PersistentMemory,
            _ => MemoryType::Unknown,
        }
    }
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

static mut BOOT_INFO: BootInfo = BootInfo {
    framebuffer: FrameBufferInfo {
        addr: 0,
        width: 0,
        height: 0,
        stride: 0,
        _padding: 0,
    },
    memory_map_count: 0,
    memory_map: core::ptr::null(),
};

const MEMORY_MAP_STORAGE_LEN: usize = 256;
static mut MEMORY_MAP_STORAGE: [MemoryDescriptor; 256] = [MemoryDescriptor {
    memory_type: MemoryType::Reserved,
    physical_start: 0,
    virtual_start: 0,
    page_count: 0,
    attribute: 0,
}; 256];

// Page table entry flags
const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_WRITABLE: u64 = 1 << 1;
const PAGE_HUGE: u64 = 1 << 7; // 2MB/1GB huge page

/// Page table configuration for deferred initialization
struct PageTableConfig {
    pml4: u64,
    pdpt_low: u64,  // for low-address identity mapping
    pdpt_high: u64, // for higher-half kernel mapping
    pd_low_base: u64,
    pd_high: u64,
    pt_base: u64,
    kernel_phys: u64,
    kernel_4k_pages: usize,
    pt_count: usize,
}

/// Allocate page-table memory (call before exit_boot_services)
unsafe fn allocate_page_tables(kernel_phys: u64, kernel_size: usize) -> PageTableConfig {
    let kernel_4k_pages = (kernel_size + PAGE_SIZE - 1) / PAGE_SIZE;
    let pt_count = (kernel_4k_pages + 511) / 512;

    // PML4 + PDPT_LOW + PDPT_HIGH + PD_LOW[4] + PD_HIGH + PT[n]
    let total_pages = 8 + pt_count;
    let pages_ptr = uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        UefiMemoryType::LOADER_DATA,
        total_pages,
    )
    .expect("Failed to allocate page tables");

    let base = pages_ptr.as_ptr() as u64;

    PageTableConfig {
        pml4: base,
        pdpt_low: base + PAGE_SIZE as u64,
        pdpt_high: base + 2 * PAGE_SIZE as u64,
        pd_low_base: base + 3 * PAGE_SIZE as u64,
        pd_high: base + 7 * PAGE_SIZE as u64,
        pt_base: base + 8 * PAGE_SIZE as u64,
        kernel_phys,
        kernel_4k_pages,
        pt_count,
    }
}

// Serial port output for debugging after exit_boot_services
fn serial_out(c: u8) {
    let port: u16 = 0x3F8;
    // Wait for transmit buffer empty
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
    let pdpt_high = cfg.pdpt_high as *mut u64;
    let pd_low_base = cfg.pd_low_base;
    let pd_high = cfg.pd_high as *mut u64;
    let pt_base = cfg.pt_base;

    let total_pages = 8 + cfg.pt_count;
    serial_str("[PT] Initializing page tables...\r\n");

    // Rust 2024: even inside `unsafe fn`, unsafe ops must be in `unsafe {}`.
    unsafe {
        // Zero all page tables
        core::ptr::write_bytes(pml4 as *mut u8, 0, PAGE_SIZE * total_pages);

        // PML4[0] -> PDPT_LOW (identity mapping)
        *pml4.add(0) = cfg.pdpt_low | PAGE_PRESENT | PAGE_WRITABLE;
        // PML4[KERNEL_PML4_INDEX] -> PDPT_HIGH (kernel mapping)
        *pml4.add(KERNEL_PML4_INDEX) = cfg.pdpt_high | PAGE_PRESENT | PAGE_WRITABLE;

        serial_str("[PT] PML4[0] = ");
        serial_hex(*pml4.add(0));
        serial_str("\r\n");
        serial_str("[PT] PML4[");
        serial_hex(KERNEL_PML4_INDEX as u64);
        serial_str("] = ");
        serial_hex(*pml4.add(KERNEL_PML4_INDEX));
        serial_str("\r\n");

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

        // PDPT_HIGH[KERNEL_PDPT_INDEX] -> PD_HIGH (higher-half kernel mapping)
        *pdpt_high.add(KERNEL_PDPT_INDEX) = cfg.pd_high | PAGE_PRESENT | PAGE_WRITABLE;
        serial_str("[PT] PDPT_HIGH[");
        serial_hex(KERNEL_PDPT_INDEX as u64);
        serial_str("] = ");
        serial_hex(*pdpt_high.add(KERNEL_PDPT_INDEX));
        serial_str("\r\n");

        // PD_HIGH -> PT
        for i in 0..cfg.pt_count {
            let pt_addr = pt_base + i as u64 * PAGE_SIZE as u64;
            *pd_high.add(i) = pt_addr | PAGE_PRESENT | PAGE_WRITABLE;
        }

        serial_str("[PT] PD_HIGH[0] = ");
        serial_hex(*pd_high.add(0));
        serial_str("\r\n");

        // PT: map each 4KB page
        for i in 0..cfg.kernel_4k_pages {
            let pt_index = i / 512;
            let pte_index = i % 512;
            let pt = (pt_base + pt_index as u64 * PAGE_SIZE as u64) as *mut u64;
            let phys_addr = cfg.kernel_phys + i as u64 * PAGE_SIZE as u64;
            *pt.add(pte_index) = phys_addr | PAGE_PRESENT | PAGE_WRITABLE;
        }

        let pt = pt_base as *const u64;
        serial_str("[PT] PT[0] = ");
        serial_hex(*pt.add(0));
        serial_str("\r\n");
        serial_str("[PT] PT[1] = ");
        serial_hex(*pt.add(1));
        serial_str("\r\n");
        serial_str("[PT] kernel_4k_pages = ");
        serial_hex(cfg.kernel_4k_pages as u64);
        serial_str("\r\n");
        serial_str("[PT] Done!\r\n");
    }

    cfg.pml4
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("Bootloader is running");

    // Load filesystem
    let simple_file_system_handle = get_handle_for_protocol::<SimpleFileSystem>().unwrap();
    let mut simple_file_system_protocol =
        open_protocol_exclusive::<SimpleFileSystem>(simple_file_system_handle).unwrap();
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

    // Compute the offset relative to the base
    let kernel_offset = min_virt - KERNEL_VIRT_BASE;
    let total_size = (max_virt - min_virt) as usize;
    let num_pages = (total_size + PAGE_SIZE - 1) / PAGE_SIZE;

    info!("Kernel virtual range: {:#x} - {:#x}", min_virt, max_virt);
    info!(
        "Kernel offset from base: {:#x}, size: {} pages",
        kernel_offset, num_pages
    );

    // Allocate physical memory (2MB-aligned for huge pages)
    let num_pages_aligned = ((total_size + 0x200000 - 1) / 0x200000) * 512; // 2MB alignment
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
    info!(
        "  kernel_phys_base: {:#x}, total_size: {:#x}",
        kernel_phys_base, total_size
    );
    let pt_config = unsafe { allocate_page_tables(kernel_phys_base, total_size) };
    info!("  Page table memory allocated at: {:#x}", pt_config.pml4);

    // Get graphics info
    let gop_handler = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handler).unwrap();

    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();
    let stride = mode_info.stride();
    let fb_addr = gop.frame_buffer().as_mut_ptr() as u64;

    info!(
        "Screen resolution: {}x{}, stride: {}",
        width, height, stride
    );
    info!("Framebuffer address: {:#x}", fb_addr);

    unsafe {
        BOOT_INFO.framebuffer = FrameBufferInfo {
            addr: fb_addr,
            width: width as u32,
            height: height as u32,
            stride: stride as u32,
            _padding: 0,
        };
    }

    // Exit boot services
    info!("Exiting boot services...");
    let memory_map = unsafe { uefi::boot::exit_boot_services(UefiMemoryType::LOADER_DATA) };

    // Convert memory map
    let mut count = 0usize;
    unsafe {
        for desc in memory_map.entries() {
            if count >= MEMORY_MAP_STORAGE_LEN {
                break;
            }
            MEMORY_MAP_STORAGE[count] = MemoryDescriptor {
                memory_type: MemoryType::from(desc.ty),
                physical_start: desc.phys_start,
                virtual_start: desc.virt_start,
                page_count: desc.page_count,
                attribute: desc.att.bits(),
            };
            count += 1;
        }

        BOOT_INFO.memory_map_count = count as u64;
        BOOT_INFO.memory_map = core::ptr::addr_of!(MEMORY_MAP_STORAGE) as *const MemoryDescriptor;
    }

    // Initialize page tables after exit_boot_services
    let pml4_phys = unsafe { init_page_tables(&pt_config) };

    // Manually verify page-table translation
    unsafe {
        serial_str("[PT] Verifying page table for entry point...\r\n");

        let pml4 = pml4_phys as *const u64;
        let pml4_entry = *pml4.add(KERNEL_PML4_INDEX);
        serial_str("[PT] Walk: PML4[");
        serial_hex(KERNEL_PML4_INDEX as u64);
        serial_str("] = ");
        serial_hex(pml4_entry);
        serial_str("\r\n");

        let pdpt = (pml4_entry & !0xFFF) as *const u64;
        let pdpt_entry = *pdpt.add(KERNEL_PDPT_INDEX);
        serial_str("[PT] Walk: PDPT[");
        serial_hex(KERNEL_PDPT_INDEX as u64);
        serial_str("] = ");
        serial_hex(pdpt_entry);
        serial_str("\r\n");

        let pd = (pdpt_entry & !0xFFF) as *const u64;
        let pd_0 = *pd.add(0);
        serial_str("[PT] Walk: PD[0] = ");
        serial_hex(pd_0);
        serial_str("\r\n");

        let pt = (pd_0 & !0xFFF) as *const u64;
        let pt_1 = *pt.add(1);
        serial_str("[PT] Walk: PT[1] = ");
        serial_hex(pt_1);
        serial_str("\r\n");

        let phys = pt_1 & !0xFFF;
        serial_str("[PT] Expected phys for entry point: ");
        serial_hex(phys + 0x270);
        serial_str("\r\n");

        // Verify content at the physical address
        let content = *((phys + 0x270) as *const u64);
        serial_str("[PT] Content at phys addr: ");
        serial_hex(content);
        serial_str("\r\n");

        serial_str("[PT] Jumping to virtual entry point...\r\n");
    }

    // Switch page tables and jump to kernel
    unsafe {
        let boot_info_ptr = core::ptr::addr_of!(BOOT_INFO);

        core::arch::asm!(
            // Load new CR3
            "mov cr3, {cr3}",
            // Jump to kernel entry point (using virtual address)
            "jmp {entry}",
            cr3 = in(reg) pml4_phys,
            entry = in(reg) entry_point,  // using virtual address
            in("rdi") boot_info_ptr,
            options(noreturn)
        );
    }
}
