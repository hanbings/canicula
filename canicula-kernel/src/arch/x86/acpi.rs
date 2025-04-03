use core::ptr::NonNull;

use acpi::{
    bgrt::Bgrt,
    fadt::Fadt,
    hpet::HpetTable,
    madt::{IoApicEntry, Madt},
    AcpiHandler, AcpiTables, PhysicalMapping,
};
use bootloader_api::BootInfo;
use lazy_static::lazy_static;
use log::debug;
use spin::Once;
use x86_64::{PhysAddr, VirtAddr};

use super::memory::physical_to_virtual;

#[derive(Debug, Clone, Copy)]
struct Acpi;

impl AcpiHandler for Acpi {
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

#[derive(Debug)]
pub struct AcpiQuery {
    io_apic: IoApicEntry,
}

lazy_static! {
    pub static ref ACPI: Once<AcpiQuery> = Once::new();
}

pub fn init(boot_info: &BootInfo) {
    let acpi_headler = Acpi;
    let rsdp: usize = boot_info.rsdp_addr.into_option().unwrap() as usize;

    unsafe {
        let tables = AcpiTables::from_rsdp(acpi_headler, rsdp).unwrap();

        let bgrt = tables.find_table::<Bgrt>().unwrap();
        let fadt = tables.find_table::<Fadt>().unwrap();
        let hpet = tables.find_table::<HpetTable>().unwrap();
        let madt = tables.find_table::<Madt>().unwrap();

        for entry in madt.get().entries() {
            match entry {
                acpi::madt::MadtEntry::LocalApic(_local_apic_entry) => {}
                acpi::madt::MadtEntry::IoApic(io_apic_entry) => {
                    ACPI.call_once(|| AcpiQuery {
                        io_apic: *io_apic_entry,
                    });
                }
                acpi::madt::MadtEntry::InterruptSourceOverride(
                    _interrupt_source_override_entry,
                ) => {}
                acpi::madt::MadtEntry::NmiSource(_nmi_source_entry) => {}
                acpi::madt::MadtEntry::LocalApicNmi(_local_apic_nmi_entry) => {}
                acpi::madt::MadtEntry::LocalApicAddressOverride(
                    _local_apic_address_override_entry,
                ) => {}
                acpi::madt::MadtEntry::IoSapic(_io_sapic_entry) => {}
                acpi::madt::MadtEntry::LocalSapic(_local_sapic_entry) => {}
                acpi::madt::MadtEntry::PlatformInterruptSource(
                    _platform_interrupt_source_entry,
                ) => {}
                acpi::madt::MadtEntry::LocalX2Apic(_local_x2_apic_entry) => {}
                acpi::madt::MadtEntry::X2ApicNmi(_x2_apic_nmi_entry) => {}
                acpi::madt::MadtEntry::Gicc(_gicc_entry) => {}
                acpi::madt::MadtEntry::Gicd(_gicd_entry) => {}
                acpi::madt::MadtEntry::GicMsiFrame(_gic_msi_frame_entry) => {}
                acpi::madt::MadtEntry::GicRedistributor(_gic_redistributor_entry) => {}
                acpi::madt::MadtEntry::GicInterruptTranslationService(
                    _gic_interrupt_translation_service_entry,
                ) => {}
                acpi::madt::MadtEntry::MultiprocessorWakeup(_multiprocessor_wakeup_entry) => {}
            }

            debug!("madt entry: {:#?}", entry);
        }

        debug!("acpi bgrt: {:#?}", bgrt);
        debug!("acpi fadt: {:#?}", fadt);
        debug!("acpi hpet: {:#?}", hpet);
        debug!("acpi madt: {:#?}", madt);
    }
}

pub fn get_io_apic_address() -> VirtAddr {
    let io_apic = ACPI.get().unwrap().io_apic;
    let io_apic_address = io_apic.io_apic_address;
    let io_apic_address = unsafe { physical_to_virtual(PhysAddr::new(io_apic_address.into())) };

    VirtAddr::new(io_apic_address.as_u64())
}
