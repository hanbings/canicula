pub mod handler;

use acpi::{AcpiTables, sdt::fadt::Fadt};
use aml::{AmlContext, AmlName, AmlValue, DebugVerbosity};
use lazy_static::lazy_static;
use log::{debug, error, warn};
use spin::Mutex;
use x86_64::{PhysAddr, instructions::port::Port};

extern crate alloc;
use alloc::boxed::Box;

#[derive(Debug, Clone, Copy)]
pub struct AcpiShutdown {
    pub pm1a_control_block: u64,
    pub slp_typ_a: u16,
    pub slp_len: u16,
    pub use_qemu_fallback: bool,
}

lazy_static! {
    pub static ref ACPI_SHUTDOWN: Mutex<AcpiShutdown> = Mutex::new(AcpiShutdown {
        pm1a_control_block: 0,
        slp_typ_a: 0,
        slp_len: 0,
        use_qemu_fallback: true,
    });
}

pub fn init(rsdp_addr: &u64) {
    let tables = match unsafe {
        AcpiTables::from_rsdp(
            crate::arch::x86::acpi::handler::AcpiHandler,
            *rsdp_addr as usize,
        )
    } {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to parse ACPI tables: {:?}", e);
            warn!("Using QEMU fallback for shutdown");
            return;
        }
    };

    let dsdt = match tables.dsdt() {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to get DSDT table: {:?}", e);
            warn!("Using QEMU fallback for shutdown");
            return;
        }
    };

    let fadt = match tables.find_table::<Fadt>() {
        Some(f) => f,
        None => {
            error!("Failed to get FADT table");
            warn!("Using QEMU fallback for shutdown");
            return;
        }
    };

    let pm1a_control_block = match fadt.pm1a_control_block() {
        Ok(p) => p,
        Err(e) => {
            error!("PM1A control block not found in FADT: {:?}", e);
            warn!("Using QEMU fallback for shutdown");
            return;
        }
    };

    // Try to parse AML and get S5 sleep type
    let slp_typ_a = {
        // ACPI table header is 36 bytes, AML code starts after it
        const ACPI_TABLE_HEADER_SIZE: usize = 36;
        let table = unsafe {
            let ptr = crate::arch::x86::memory::physical_to_virtual(PhysAddr::new(
                dsdt.phys_address as u64,
            ));
            // Skip the ACPI table header to get pure AML bytecode
            let aml_start = ptr.as_ptr::<u8>().add(ACPI_TABLE_HEADER_SIZE);
            let aml_length = dsdt.length as usize - ACPI_TABLE_HEADER_SIZE;
            core::slice::from_raw_parts(aml_start, aml_length)
        };

        let handler = Box::new(crate::arch::x86::acpi::handler::AmlHandler);
        let mut aml = AmlContext::new(handler, DebugVerbosity::None);

        if let Err(e) = aml.parse_table(table) {
            error!("Failed to parse AML stream. Err = {:?}", e);
            warn!("Using QEMU fallback for shutdown");
            return;
        }

        let name = match AmlName::from_str("\\_S5") {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to create AML name: {:?}", e);
                warn!("Using QEMU fallback for shutdown");
                return;
            }
        };

        match aml.namespace.get_by_path(&name) {
            Ok(AmlValue::Package(p)) => match p.first() {
                Some(AmlValue::Integer(v)) => *v as u16,
                _ => {
                    error!("\\_S5[0] is not an Integer");
                    warn!("Using QEMU fallback for shutdown");
                    return;
                }
            },
            Ok(_) => {
                error!("\\_S5 is not a Package");
                warn!("Using QEMU fallback for shutdown");
                return;
            }
            Err(e) => {
                error!("Failed to get \\_S5: {:?}", e);
                warn!("Using QEMU fallback for shutdown");
                return;
            }
        }
    };

    let slp_len = 1 << 13;

    *ACPI_SHUTDOWN.lock() = AcpiShutdown {
        pm1a_control_block: pm1a_control_block.address,
        slp_typ_a,
        slp_len,
        use_qemu_fallback: false,
    };

    debug!("PM1A Control Block: {:#x}", pm1a_control_block.address);
    debug!("S5 Sleep Type: {:#x}", slp_typ_a);
    debug!("S5 Sleep Length: {:#x}", slp_len);
}

pub fn shutdown() {
    let shutdown_info = ACPI_SHUTDOWN.lock();

    if shutdown_info.use_qemu_fallback {
        // QEMU exit: write to debug exit port (0xf4)
        // Exit code will be (value << 1) | 1
        debug!("Using QEMU debug exit for shutdown");
        unsafe {
            let mut port: Port<u32> = Port::new(0xf4);
            port.write(0x10); // Exit code will be 0x21 (33)
        }
    } else {
        // Use ACPI S5 sleep state for shutdown
        debug!("Using ACPI S5 sleep for shutdown");
        unsafe {
            let mut port: Port<u16> = Port::new(shutdown_info.pm1a_control_block as u16);
            port.write(shutdown_info.slp_typ_a | shutdown_info.slp_len);
        }
    }
}
