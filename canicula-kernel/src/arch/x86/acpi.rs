use super::memory::physical_to_virtual;
use bootloader_api::BootInfo;
use log::{debug, warn};
use x86_64::PhysAddr;

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

pub fn init(boot_info: &BootInfo) -> (Rsdp, Rsdt) {
    let rsdp_addr = *boot_info.rsdp_addr.as_ref().unwrap();
    let rsdp = unsafe { physical_to_virtual(PhysAddr::new(rsdp_addr)).as_ptr() } as *const Rsdp;
    unsafe {
        warn!("find rsdp! {:?}", (*rsdp));
    }

    let rsdt_addr: u64 = unsafe { (*rsdp).rsdt_address }.into();
    let rsdt = unsafe { physical_to_virtual(PhysAddr::new(rsdt_addr)).as_ptr() } as *const Rsdt;

    unsafe {
        warn!("find rsdt! {:?}", (*rsdt));
        let num_tables =
            ((*rsdt).header.length - core::mem::size_of::<AcpiTableHeader>() as u32) / 4;

        for i in 0..num_tables {
            let table_addr = (*rsdt).tables.as_ptr().add(i as usize);
            debug!("Table {} Address: {:?}", i + 1, table_addr);
        }
    }

    unsafe { ((*rsdp), (*rsdt)) }
}
