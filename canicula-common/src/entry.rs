pub trait KernelEntry {
    fn entry() -> !;
}

/// Boot information passed from the bootloader to the kernel.
#[derive(Debug)]
#[repr(C)]
pub struct BootInfo {
    /// The memory regions provided by the bootloader.
    pub memory_regions: MemoryRegions,
    /// Information about the framebuffer, if available.
    pub framebuffer: Option<FrameBuffer>,
    /// Physical memory offset for direct mapping.
    pub physical_memory_offset: Option<u64>,
    /// RSDP address for ACPI, if available.
    pub rsdp_addr: Option<u64>,
}

impl BootInfo {
    pub const fn new() -> Self {
        Self {
            memory_regions: MemoryRegions::new(),
            framebuffer: None,
            physical_memory_offset: None,
            rsdp_addr: None,
        }
    }
}

/// Memory region information.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryRegion {
    /// Start physical address of the region.
    pub start: u64,
    /// End physical address of the region (exclusive).
    pub end: u64,
    /// The kind of memory region.
    pub kind: MemoryRegionKind,
}

/// The kind of a memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum MemoryRegionKind {
    /// Usable memory that can be freely used by the kernel.
    Usable,
    /// Memory reserved by the bootloader.
    Bootloader,
    /// Memory used for UEFI runtime services.
    UnknownUefi(u32),
    /// Memory used for BIOS or other firmware.
    UnknownBios(u32),
}

/// A collection of memory regions.
#[derive(Debug)]
#[repr(C)]
pub struct MemoryRegions {
    regions: [MemoryRegion; 256],
    len: usize,
}

impl MemoryRegions {
    pub const fn new() -> Self {
        Self {
            regions: [MemoryRegion {
                start: 0,
                end: 0,
                kind: MemoryRegionKind::Usable,
            }; 256],
            len: 0,
        }
    }

    pub fn push(&mut self, region: MemoryRegion) {
        if self.len < self.regions.len() {
            self.regions[self.len] = region;
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemoryRegion> {
        self.regions[..self.len].iter()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Framebuffer information.
#[derive(Debug)]
#[repr(C)]
pub struct FrameBuffer {
    /// Pointer to the framebuffer memory.
    buffer_start: u64,
    /// Size of the framebuffer in bytes.
    buffer_size: usize,
    /// Framebuffer metadata.
    info: FrameBufferInfo,
}

impl FrameBuffer {
    pub const fn new(buffer_start: u64, buffer_size: usize, info: FrameBufferInfo) -> Self {
        Self {
            buffer_start,
            buffer_size,
            info,
        }
    }

    /// Returns the framebuffer as a mutable byte slice.
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.buffer_start as *mut u8, self.buffer_size) }
    }

    /// Returns the framebuffer as a byte slice.
    pub fn buffer(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.buffer_start as *const u8, self.buffer_size) }
    }

    /// Returns framebuffer metadata.
    pub fn info(&self) -> FrameBufferInfo {
        self.info
    }
}

/// Framebuffer metadata.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FrameBufferInfo {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Stride (pixels per row, may include padding).
    pub stride: usize,
    /// Bytes per pixel.
    pub bytes_per_pixel: usize,
    /// Pixel format.
    pub pixel_format: PixelFormat,
}

/// Pixel format for the framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum PixelFormat {
    /// RGB format (red, green, blue).
    Rgb,
    /// BGR format (blue, green, red).
    Bgr,
    /// Unknown pixel format.
    Unknown {
        red_position: u8,
        green_position: u8,
        blue_position: u8,
    },
}
