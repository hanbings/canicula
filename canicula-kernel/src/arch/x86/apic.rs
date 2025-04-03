use log::warn;
use x86::apic::{x2apic::X2APIC, ApicControl};

pub fn init() {
    let mut x2apic = X2APIC::new();
    x2apic.attach();

    warn!(
        "x2apic initialized id {} logic id {}, bsp {}",
        x2apic.id(),
        x2apic.logical_id(),
        x2apic.bsp()
    );
}
