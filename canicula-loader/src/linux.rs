extern crate alloc;

use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use log::info;
use uefi::boot::LoadImageSource;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{CStr16, Status};

use crate::config::{CMDLINE, INITRD_PATH, VMLINUZ_PATH};
use crate::FILE_BUFFER_SIZE;

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
    vendor_type: u8,        // 0x04 = MEDIA_DEVICE_PATH
    vendor_subtype: u8,     // 0x03 = MEDIA_VENDOR_DP
    vendor_length: [u8; 2], // 20 = sizeof(header) + sizeof(GUID)
    vendor_guid: [u8; 16],  // LINUX_EFI_INITRD_MEDIA_GUID in mixed-endian
    // End of Entire Device Path node
    end_type: u8,        // 0x7F
    end_subtype: u8,     // 0xFF
    end_length: [u8; 2], // 4
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
        0xfc, 0x68, // next 2 bytes LE
        0x3d, 0x4f, // next 2 bytes LE
        0xac, 0x74, // next 8 bytes BE
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
const DEVICE_PATH_PROTOCOL_GUID: uefi::Guid = uefi::guid!("09576e91-6d3f-11d2-8e39-00a0c969723b");
const LOAD_FILE2_PROTOCOL_GUID: uefi::Guid = uefi::guid!("4006c0c1-fcb3-403e-996d-4a6c8724e06d");

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
pub fn boot_linux_efi_stub() -> Status {
    info!("Linux EFI Stub Boot");

    // Read vmlinuz and initrd from the ESP
    let vmlinuz_data: alloc::vec::Vec<u8>;
    let initrd_data: Option<alloc::vec::Vec<u8>>;

    {
        let sfs_handle = uefi::boot::get_handle_for_protocol::<SimpleFileSystem>().unwrap();
        let mut sfs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(sfs_handle).unwrap();
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
            let initrd_path = CStr16::from_str_with_buf(INITRD_PATH, &mut initrd_path_buf).ok()?;
            let initrd_handle = root
                .open(initrd_path, FileMode::Read, FileAttribute::empty())
                .ok()?;
            let mut initrd_file = match initrd_handle.into_type().ok()? {
                FileType::Regular(f) => f,
                _ => return None,
            };
            let mut initrd_info_buf = [0u8; FILE_BUFFER_SIZE];
            let initrd_info: &mut FileInfo = initrd_file.get_info(&mut initrd_info_buf).ok()?;
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
    let cmdline_size = (cmdline.to_u16_slice_with_nul().len() * core::mem::size_of::<u16>()) as u32;

    {
        let mut loaded_image = uefi::boot::open_protocol_exclusive::<LoadedImage>(image_handle)
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
