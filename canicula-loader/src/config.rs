/// Boot mode selection: which kernel to boot
#[derive(PartialEq, Clone, Copy)]
pub enum BootMode {
    /// Boot the custom Canicula kernel (ELF format)
    #[allow(dead_code)]
    CaniculaKernel,
    /// Boot a standard Linux kernel via EFI stub (PE/COFF vmlinuz)
    #[allow(dead_code)]
    LinuxEfiStub,
}

/// A boot menu entry
pub struct BootEntry {
    /// Display name shown in the boot menu
    pub name: &'static str,
    /// Boot mode to use when this entry is selected
    pub mode: BootMode,
}

// Boot menu configuration

/// Available boot entries shown in the boot menu
pub static BOOT_ENTRIES: &[BootEntry] = &[
    BootEntry {
        name: "Canicula Kernel",
        mode: BootMode::CaniculaKernel,
    },
    BootEntry {
        name: "Linux (EFI Stub)",
        mode: BootMode::LinuxEfiStub,
    },
];

/// Default selected entry index (0-based)
pub const DEFAULT_ENTRY: usize = 0;

/// Auto-boot timeout in seconds
pub const BOOT_TIMEOUT_SECS: usize = 5;

// Linux EFI Stub boot configuration

/// Path to the Linux kernel image (vmlinuz) on the EFI System Partition
pub static VMLINUZ_PATH: &str = "\\vmlinuz";

/// Path to the initial ramdisk image on the EFI System Partition
pub static INITRD_PATH: &str = "\\initrd.img";

/// Kernel command line passed to the Linux kernel
pub static CMDLINE: &str = "console=tty0 console=ttyS0";

// Canicula kernel boot configuration

/// Path to the Canicula kernel ELF binary on the EFI System Partition
pub static KERNEL_PATH: &str = "\\kernel-x86_64";

/// Physical memory direct mapping base address
/// Identity maps the first 4GB starting at this virtual address
pub const PHYSICAL_MEMORY_OFFSET: u64 = 0xffff_8800_0000_0000;

/// PML4 page table index for the kernel mapping (virtual address 0xfffff80000000000)
pub const KERNEL_PML4_INDEX: usize = 496; // (0xfffff80000000000 >> 39) & 0x1FF

/// PML4 page table index for the physical memory direct mapping
pub const PHYS_MAP_PML4_INDEX: usize = 272; // 0xffff880000000000 >> 39 & 0x1FF
