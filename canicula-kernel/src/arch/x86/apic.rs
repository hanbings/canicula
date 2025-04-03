use log::info;
use x86::apic::{ioapic::IoApic, x2apic::X2APIC, ApicControl};

use crate::arch::x86::acpi::get_io_apic_address;

pub fn init() {
    let mut x2apic = X2APIC::new();
    x2apic.attach();

    info!(
        "x2apic initialized id {} logic id {}, bsp {}",
        x2apic.id(),
        x2apic.logical_id(),
        x2apic.bsp()
    );

    let mut ioapic = unsafe { IoApic::new(get_io_apic_address().as_u64() as usize) };
    info!("IO APIC address: {:#x}", get_io_apic_address());
    info!(
        "IO APIC id: {}, version: {}, supported interrupts: {:?}",
        ioapic.id(),
        ioapic.version(),
        ioapic.supported_interrupts()
    );
}
