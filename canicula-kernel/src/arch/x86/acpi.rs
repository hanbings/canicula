use core::ptr::NonNull;

use acpi::{
    bgrt::Bgrt, fadt::Fadt, hpet::HpetTable, madt::Madt, AcpiHandler, AcpiTables, PhysicalMapping,
};
use x86_64::PhysAddr;

#[derive(Debug, Clone, Copy)]
pub struct Handler;

impl AcpiHandler for Handler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let phys_addr = PhysAddr::new(physical_address as u64);
        let virt_addr = crate::arch::x86::memory::physical_to_virtual(phys_addr);
        let ptr = NonNull::new(virt_addr.as_mut_ptr()).unwrap();
        PhysicalMapping::new(physical_address, ptr, size, size, Self)
    }

    fn unmap_physical_region<T>(_region: &acpi::PhysicalMapping<Self, T>) {}
}

pub fn init(rsdp_addr: &u64) {
    let tables = unsafe {
        AcpiTables::from_rsdp(crate::arch::x86::acpi::Handler, *rsdp_addr as usize).unwrap()
    };
    let _platform_info = tables.platform_info().unwrap();

    let _bgrt = tables
        .find_table::<Bgrt>()
        .unwrap_or_else(|_| panic!("Failed to get BGR table"));
    let _hpet = tables
        .find_table::<HpetTable>()
        .unwrap_or_else(|_| panic!("Failed to get HPET table"));
    let _fadt = tables
        .find_table::<Fadt>()
        .unwrap_or_else(|_| panic!("Failed to get FADT table"));
    let _madt = tables
        .find_table::<Madt>()
        .unwrap_or_else(|_| panic!("Failed to get MADT table"));
}
