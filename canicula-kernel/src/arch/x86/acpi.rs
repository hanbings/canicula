use core::ptr::NonNull;

use acpi::{AcpiHandler, PhysicalMapping};
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
