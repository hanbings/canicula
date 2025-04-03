use bootloader_api::BootInfo;
use log::warn;
use x86_64::{
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct Rsdp {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,

    /*
     * These fields are only valid for ACPI Version 2.0 and greater
     */
    length: u32,
    xsdt_address: u64,
    ext_checksum: u8,
    reserved: [u8; 3],
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct Rsdt {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oemtableid: [u8; 8],
    oemrevision: u32,
    creatorid: u32,
    creatorrevision: u32,
    tables: [u32; 0],
}

pub fn init(
    boot_info: &BootInfo,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> (Rsdp, Rsdt) {
    let rsdp_addr = boot_info.rsdp_addr.as_ref().unwrap();

    unsafe {
        let _ = mapper.map_to(
            Page::<Size4KiB>::from_start_address(VirtAddr::new(0x1000_0000_0000)).unwrap(),
            PhysFrame::containing_address(PhysAddr::new(*rsdp_addr)),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            frame_allocator,
        );
    };

    let rsdp = 0x1000_0000_0000 as *const Rsdp;
    unsafe {
        warn!("find rsdp! {:?}", (*rsdp));
    }

    let rsdt_addr = unsafe { (*rsdp).rsdt_address } as u64;
    unsafe {
        let _ = mapper.map_to(
            Page::<Size4KiB>::from_start_address(VirtAddr::new(0x2000_0000_0000)).unwrap(),
            PhysFrame::containing_address(PhysAddr::new(rsdt_addr)),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            frame_allocator,
        );
    };

    let rsdt = 0x2000_0000_0000 as *const Rsdt;
    unsafe { warn!("find rsdt! {:?}", (*rsdt)) }

    unsafe { ((*rsdp), (*rsdt)) }
}
