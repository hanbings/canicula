use bootloader_api::BootInfo;
use log::{debug, warn};
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
#[repr(C)]
pub struct Rsdt {
    header: AcpiTableHeader,
    tables: [u32; 0],
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct AcpiTableHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oemtableid: [u8; 8],
    oemrevision: u32,
    creatorid: u32,
    creatorrevision: u32,
}

pub fn init(
    boot_info: &BootInfo,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> (Rsdp, Rsdt) {
    let rsdp_addr = boot_info.rsdp_addr.as_ref().unwrap();

    unsafe {
        let _ = mapper.map_to(
            Page::<Size4KiB>::from_start_address(VirtAddr::new(*rsdp_addr).align_down(4096 as u32)).unwrap(),
            PhysFrame::containing_address(PhysAddr::new(*rsdp_addr)),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            frame_allocator,
        );
    };

    let rsdp = (*rsdp_addr) as *const Rsdp;
    unsafe {
        warn!("find rsdp! {:?}", (*rsdp));
    }

    let rsdt_addr: u64 = unsafe { (*rsdp).rsdt_address }.into();
    unsafe {
        let _ = mapper.map_to(
            Page::<Size4KiB>::from_start_address(VirtAddr::new(rsdt_addr).align_down(4096 as u32)).unwrap(),
            PhysFrame::containing_address(PhysAddr::new(rsdt_addr)),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            frame_allocator,
        );
    };

    let rsdt = rsdt_addr as *const Rsdt;
    unsafe {
        warn!("find rsdt! {:?}", (*rsdt));
        let num_tables = ((*rsdt).header.length - core::mem::size_of::<AcpiTableHeader>() as u32) / 4;

        for i in 0..num_tables {
            let table_addr = (*rsdt).tables.as_ptr().add(i as usize);
            debug!("Table {} Address: {:?}", i + 1, table_addr);

            let header_ptr = table_addr as *const AcpiTableHeader;
            let header = *header_ptr;
            debug!(
                "ACPI Table Header: sig={:?} len={} rev={} checksum={} oemid={:?}",
                core::str::from_utf8(&header.signature).unwrap_or("???"),
                header.length,
                header.revision,
                header.checksum,
                header.oemid
            )
        }
    }

    unsafe { ((*rsdp), (*rsdt)) }
}
