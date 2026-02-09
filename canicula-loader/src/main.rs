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
use uefi::boot::LoadImageSource;
use uefi::proto::loaded_image::LoadedImage;
use xmas_elf::ElfFile;
use xmas_elf::program::Type;

use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

static KERNEL_PATH: &str = "\\kernel-x86_64";
static FILE_BUFFER_SIZE: usize = 0x400;
static PAGE_SIZE: usize = 0x1000;

// Set BOOT_MODE to select which kernel to boot
#[derive(PartialEq)]
enum BootMode {
    /// Boot the custom Canicula kernel (ELF format)
    #[allow(dead_code)]
    CaniculaKernel,
    /// Boot a standard Linux kernel via EFI stub (PE/COFF vmlinuz)
    #[allow(dead_code)]
    LinuxEfiStub,
}

const BOOT_MODE: BootMode = BootMode::LinuxEfiStub;

// Linux EFI stub boot configuration
static VMLINUZ_PATH: &str = "\\vmlinuz";
static INITRD_PATH: &str = "\\initrd.img";
static CMDLINE: &str = "console=ttyS0";

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

/// Global initrd data pointer and length, set before installing the LoadFile2 protocol.
/// Accessed by the LoadFile2 callback when the Linux kernel requests the initrd.
static INITRD_DATA_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static INITRD_DATA_LEN: AtomicUsize = AtomicUsize::new(0);

/// Device path identifying the Linux initrd media.
/// Layout: Vendor Media Device Path node + End of Device Path node.
/// The Linux EFI stub (5.8+) searches for a handle with this device path
/// and the LoadFile2 protocol to load the initrd.
#[repr(C, packed)]
struct InitrdDevicePath {
    // Vendor Media Device Path node
    vendor_type: u8,         // 0x04 = MEDIA_DEVICE_PATH
    vendor_subtype: u8,      // 0x03 = MEDIA_VENDOR_DP
    vendor_length: [u8; 2],  // 20 = sizeof(header) + sizeof(GUID)
    vendor_guid: [u8; 16],   // LINUX_EFI_INITRD_MEDIA_GUID in mixed-endian
    // End of Entire Device Path node
    end_type: u8,            // 0x7F
    end_subtype: u8,         // 0xFF
    end_length: [u8; 2],     // 4
}

// Safety: InitrdDevicePath is composed entirely of primitive types.
unsafe impl Sync for InitrdDevicePath {}

/// Static device path instance: LINUX_EFI_INITRD_MEDIA_GUID
/// {5568e427-68fc-4f3d-ac74-ca555231cc68}
static INITRD_DEVICE_PATH: InitrdDevicePath = InitrdDevicePath {
    vendor_type: 0x04,
    vendor_subtype: 0x03,
    vendor_length: [20, 0],
    vendor_guid: [
        0x27, 0xe4, 0x68, 0x55, // first 4 bytes LE
        0xfc, 0x68,             // next 2 bytes LE
        0x3d, 0x4f,             // next 2 bytes LE
        0xac, 0x74,             // next 8 bytes BE
        0xca, 0x55, 0x52, 0x31, 0xcc, 0x68,
    ],
    end_type: 0x7f,
    end_subtype: 0xff,
    end_length: [4, 0],
};

/// Raw EFI_LOAD_FILE2_PROTOCOL struct (ABI-compatible with UEFI specification)
#[repr(C)]
struct RawLoadFile2Protocol {
    load_file: unsafe extern "efiapi" fn(
        this: *mut RawLoadFile2Protocol,
        file_path: *const c_void,
        boot_policy: bool,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> uefi::Status,
}

// Safety: RawLoadFile2Protocol contains only a function pointer, which is Send+Sync.
unsafe impl Sync for RawLoadFile2Protocol {}

/// LoadFile2 callback: provides the initrd data to the Linux EFI stub.
/// Called by the kernel's EFI stub during boot when it needs the initrd.
unsafe extern "efiapi" fn initrd_load_file(
    _this: *mut RawLoadFile2Protocol,
    _file_path: *const c_void,
    _boot_policy: bool,
    buffer_size: *mut usize,
    buffer: *mut c_void,
) -> uefi::Status {
    let ptr = INITRD_DATA_PTR.load(Ordering::Relaxed);
    let len = INITRD_DATA_LEN.load(Ordering::Relaxed);

    if ptr.is_null() || len == 0 {
        return uefi::Status::NOT_FOUND;
    }

    unsafe {
        if buffer.is_null() || *buffer_size < len {
            *buffer_size = len;
            return uefi::Status::BUFFER_TOO_SMALL;
        }

        core::ptr::copy_nonoverlapping(ptr, buffer as *mut u8, len);
        *buffer_size = len;
    }

    uefi::Status::SUCCESS
}

/// Static LoadFile2 protocol instance with the initrd callback
static INITRD_LOAD_FILE2: RawLoadFile2Protocol = RawLoadFile2Protocol {
    load_file: initrd_load_file,
};

// Well-known protocol GUIDs
const DEVICE_PATH_PROTOCOL_GUID: uefi::Guid =
    uefi::guid!("09576e91-6d3f-11d2-8e39-00a0c969723b");
const LOAD_FILE2_PROTOCOL_GUID: uefi::Guid =
    uefi::guid!("4006c0c1-fcb3-403e-996d-4a6c8724e06d");

/// Install the initrd LoadFile2 protocol on a new UEFI handle.
///
/// Creates a handle with:
///   1. EFI_DEVICE_PATH_PROTOCOL pointing to LINUX_EFI_INITRD_MEDIA_GUID vendor path
///   2. EFI_LOAD_FILE2_PROTOCOL that serves the initrd data
///
/// The Linux EFI stub (kernel 5.8+) discovers this protocol to load the initrd.
fn install_initrd_load_file2(initrd_data: &[u8]) {
    // Store the initrd data pointer for the callback
    INITRD_DATA_PTR.store(initrd_data.as_ptr() as *mut u8, Ordering::Relaxed);
    INITRD_DATA_LEN.store(initrd_data.len(), Ordering::Relaxed);

    // Install device path protocol on a new handle
    let handle = unsafe {
        uefi::boot::install_protocol_interface(
            None,
            &DEVICE_PATH_PROTOCOL_GUID,
            &INITRD_DEVICE_PATH as *const InitrdDevicePath as *const c_void,
        )
    }
    .expect("Failed to install initrd device path protocol");

    // Install LoadFile2 protocol on the same handle
    unsafe {
        uefi::boot::install_protocol_interface(
            Some(handle),
            &LOAD_FILE2_PROTOCOL_GUID,
            &INITRD_LOAD_FILE2 as *const RawLoadFile2Protocol as *const c_void,
        )
    }
    .expect("Failed to install initrd LoadFile2 protocol");
}

/// Boot a Linux kernel (vmlinuz) via the EFI stub mechanism.
///
/// This function:
/// 1. Reads vmlinuz from the EFI System Partition
/// 2. Optionally reads an initrd and installs a LoadFile2 protocol for it
/// 3. Loads the vmlinuz as a UEFI image via LoadImage
/// 4. Sets the kernel command line via the LoadedImage protocol
/// 5. Starts the kernel via StartImage
fn boot_linux_efi_stub() -> Status {
    info!("Linux EFI Stub Boot");

    // Read vmlinuz and initrd from the ESP
    let vmlinuz_data: alloc::vec::Vec<u8>;
    let initrd_data: Option<alloc::vec::Vec<u8>>;

    {
        let sfs_handle =
            uefi::boot::get_handle_for_protocol::<SimpleFileSystem>().unwrap();
        let mut sfs =
            uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(sfs_handle).unwrap();
        let mut root = sfs.open_volume().unwrap();

        // Read vmlinuz
        info!("Loading vmlinuz from {} ...", VMLINUZ_PATH);
        let mut path_buf = [0u16; FILE_BUFFER_SIZE];
        let path = CStr16::from_str_with_buf(VMLINUZ_PATH, &mut path_buf).unwrap();
        let handle = root
            .open(path, FileMode::Read, FileAttribute::empty())
            .expect("Failed to open vmlinuz");
        let mut file = match handle.into_type().unwrap() {
            FileType::Regular(f) => f,
            _ => panic!("vmlinuz is not a regular file!"),
        };

        let mut info_buf = [0u8; FILE_BUFFER_SIZE];
        let file_info: &mut FileInfo = file.get_info(&mut info_buf).unwrap();
        let file_size = usize::try_from(file_info.file_size()).unwrap();
        info!("vmlinuz size: {} bytes", file_size);

        vmlinuz_data = {
            let mut buf = alloc::vec![0u8; file_size];
            file.read(&mut buf).unwrap();
            buf
        };
        info!("vmlinuz loaded into memory");

        // Read initrd
        info!("Looking for initrd at {} ...", INITRD_PATH);
        initrd_data = (|| -> Option<alloc::vec::Vec<u8>> {
            let mut initrd_path_buf = [0u16; FILE_BUFFER_SIZE];
            let initrd_path =
                CStr16::from_str_with_buf(INITRD_PATH, &mut initrd_path_buf).ok()?;
            let initrd_handle = root
                .open(initrd_path, FileMode::Read, FileAttribute::empty())
                .ok()?;
            let mut initrd_file = match initrd_handle.into_type().ok()? {
                FileType::Regular(f) => f,
                _ => return None,
            };
            let mut initrd_info_buf = [0u8; FILE_BUFFER_SIZE];
            let initrd_info: &mut FileInfo =
                initrd_file.get_info(&mut initrd_info_buf).ok()?;
            let initrd_size = usize::try_from(initrd_info.file_size()).ok()?;
            let mut buf = alloc::vec![0u8; initrd_size];
            initrd_file.read(&mut buf).ok()?;
            info!("initrd loaded: {} bytes", initrd_size);
            Some(buf)
        })();

        if initrd_data.is_none() {
            info!("No initrd found, continuing without it");
        }
    }

    // Install initrd LoadFile2 protocol if available
    if let Some(ref initrd) = initrd_data {
        install_initrd_load_file2(initrd);
        info!("Initrd LoadFile2 protocol installed (LINUX_EFI_INITRD_MEDIA_GUID)");
    }

    // Load vmlinuz as an EFI image
    info!("Loading vmlinuz as EFI image via UEFI LoadImage...");
    let image_handle = uefi::boot::load_image(
        uefi::boot::image_handle(),
        LoadImageSource::FromBuffer {
            buffer: &vmlinuz_data,
            file_path: None,
        },
    )
    .expect("Failed to load vmlinuz as EFI image (is it a valid PE/COFF with EFI stub?)");
    info!("vmlinuz loaded as EFI image successfully");

    // Set kernel command line
    // The command line is passed as a null-terminated UCS-2 string via LoadedImage.LoadOptions
    let mut cmdline_buf = [0u16; 512];
    let cmdline = CStr16::from_str_with_buf(CMDLINE, &mut cmdline_buf).unwrap();
    let cmdline_size =
        (cmdline.to_u16_slice_with_nul().len() * core::mem::size_of::<u16>()) as u32;

    {
        let mut loaded_image =
            uefi::boot::open_protocol_exclusive::<LoadedImage>(image_handle)
                .expect("Failed to open LoadedImage protocol on vmlinuz");
        unsafe {
            loaded_image.set_load_options(cmdline_buf.as_ptr() as *const u8, cmdline_size);
        }
    }
    info!("Kernel command line: \"{}\"", CMDLINE);

    // Start the Linux kernel
    info!("Starting Linux kernel via EFI stub...");
    uefi::boot::start_image(image_handle).expect("Failed to start Linux kernel");

    // If start_image returns, the kernel exited
    panic!("Linux kernel returned unexpectedly");
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("Canicula Loader starting...");

    // Dispatch based on boot mode
    if BOOT_MODE == BootMode::LinuxEfiStub {
        return boot_linux_efi_stub();
    }

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
