pub mod handler;

use acpi::{fadt::Fadt, AcpiTables};
use aml::{AmlContext, AmlName, AmlValue, DebugVerbosity};
use lazy_static::lazy_static;
use log::debug;
use spin::Mutex;
use x86_64::{instructions::port::Port, PhysAddr};

extern crate alloc;
use alloc::boxed::Box;

#[derive(Debug, Clone, Copy)]
pub struct AcpiShutdown {
    pub pm1a_control_block: u64,
    pub slp_typ_a: u16,
    pub slp_len: u16,
}

lazy_static! {
    pub static ref ACPI_SHUTDOWN: Mutex<AcpiShutdown> = Mutex::new(AcpiShutdown {
        pm1a_control_block: 0,
        slp_typ_a: 0,
        slp_len: 0,
    });
}

pub fn init(rsdp_addr: &u64) {
    let tables = unsafe {
        AcpiTables::from_rsdp(
            crate::arch::x86::acpi::handler::AcpiHandler,
            *rsdp_addr as usize,
        )
        .unwrap()
    };

    let dsdt = tables
        .dsdt()
        .unwrap_or_else(|_| panic!("Failed to get DSDT table"));
    let fadt = tables
        .find_table::<Fadt>()
        .unwrap_or_else(|_| panic!("Failed to get FADT table"));

    let pm1a_control_block = fadt.pm1a_control_block().unwrap();
    let slp_typ_a = {
        let table = unsafe {
            let ptr = crate::arch::x86::memory::physical_to_virtual(PhysAddr::new(
                dsdt.address as u64,
            ));
            core::slice::from_raw_parts(ptr.as_ptr(), dsdt.length as usize)
        };

        let handler = Box::new(crate::arch::x86::acpi::handler::AmlHandler);
        let mut aml = AmlContext::new(handler, DebugVerbosity::None);

        let name = AmlName::from_str("\\_S5").unwrap();
        aml.parse_table(table).unwrap();

        let s5 = match aml.namespace.get_by_path(&name).unwrap() {
            AmlValue::Package(p) => p,
            _ => panic!("\\_S5 is not a Package"),
        };

        let value = match s5[0] {
            AmlValue::Integer(v) => v as u16,
            _ => panic!("\\_S5[0] is not an Integer"),
        };

        value
    };
    let slp_len = 1 << 13;

    *ACPI_SHUTDOWN.lock() = AcpiShutdown {
        pm1a_control_block: pm1a_control_block.address,
        slp_typ_a,
        slp_len,
    };

    debug!("PM1A Control Block: {:#x}", pm1a_control_block.address);
    debug!("S5 Sleep Type: {:#x}", slp_typ_a);
    debug!("S5 Sleep Length: {:#x}", slp_len);
}

pub fn shutdown() {
    let pm1a_control_block = ACPI_SHUTDOWN.lock().pm1a_control_block;
    let slp_typ_a = ACPI_SHUTDOWN.lock().slp_typ_a;
    let slp_len = ACPI_SHUTDOWN.lock().slp_len;

    unsafe {
        let mut port: Port<u16> = Port::new(pm1a_control_block as u16);
        port.write(slp_typ_a | slp_len);
    }
}
