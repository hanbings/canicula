use acpi::{
    AcpiTables,
    platform::{AcpiPlatform, InterruptModel},
};
use conquer_once::spin::OnceCell;
use log::debug;
use spin::{Mutex, Once};

extern crate alloc;
use alloc::vec::Vec;

use x2apic::{
    ioapic::{IoApic, IrqMode, RedirectionTableEntry},
    lapic::{LocalApic, LocalApicBuilder},
};
use x86_64::{PhysAddr, instructions::port::Port};

pub static IOAPIC: Once<Mutex<Vec<IOApic>>> = Once::new();
pub static mut LAPIC: OnceCell<Mutex<LApic>> = OnceCell::uninit();

pub struct IOApic {
    addr: u64,
    ioapic: Option<IoApic>,
}

pub struct LApic {
    addr: u64,
    lapic: Option<LocalApic>,
}

impl IOApic {
    pub fn new(addr: u64) -> Self {
        Self {
            addr: unsafe {
                crate::arch::x86::memory::physical_to_virtual(PhysAddr::new(addr)).as_u64()
            },
            ioapic: None,
        }
    }

    pub fn init(&mut self) {
        debug!("Initializing IOAPIC");
        self.ioapic = unsafe { Some(IoApic::new(self.addr)) };
        debug!("IOAPIC initialized");
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn enable(&mut self) {
        if let Some(ioapic) = self.ioapic.as_mut() {
            unsafe {
                ioapic.init(32);
                let mut entry = RedirectionTableEntry::default();
                entry.set_mode(IrqMode::Fixed);
                entry.set_vector(33);
                entry.set_dest(0);

                ioapic.set_table_entry(1, entry);
                ioapic.enable_irq(1);
            }
        }
    }

    pub fn get_ioapic(&self) -> Option<&IoApic> {
        self.ioapic.as_ref()
    }
}

impl LApic {
    pub fn new(addr: u64) -> Self {
        Self {
            addr: unsafe {
                crate::arch::x86::memory::physical_to_virtual(PhysAddr::new(addr)).as_u64()
            },
            lapic: None,
        }
    }

    pub fn init(&mut self) {
        unsafe {
            let mut cmd_8259a = Port::<u8>::new(0x20);
            let mut data_8259a = Port::<u8>::new(0x21);
            let mut cmd_8259b = Port::<u8>::new(0xa0);
            let mut data_8259b = Port::<u8>::new(0xa1);

            let mut spin_port = Port::<u8>::new(0x80);
            let mut spin = || spin_port.write(0);

            cmd_8259a.write(0x11);
            cmd_8259b.write(0x11);
            spin();

            data_8259a.write(0xf8);
            data_8259b.write(0xff);
            spin();

            data_8259a.write(0b100);
            spin();

            data_8259b.write(0b10);
            spin();

            data_8259a.write(0x1);
            data_8259b.write(0x1);
            spin();

            data_8259a.write(u8::MAX);
            data_8259b.write(u8::MAX);
        }

        self.lapic = LocalApicBuilder::default()
            .timer_vector(32)
            .error_vector(51)
            .spurious_vector(0xff)
            .set_xapic_base(self.addr)
            .build()
            .ok();
    }

    pub fn enable(&mut self) {
        unsafe {
            self.lapic.as_mut().unwrap().enable();
        }
    }

    pub fn id(&self) -> u32 {
        unsafe { self.lapic.as_ref().unwrap().id() }
    }

    pub fn is_bsp(&self) -> bool {
        unsafe { self.lapic.as_ref().unwrap().is_bsp() }
    }

    pub fn end_interrupts(&mut self) {
        unsafe {
            self.lapic.as_mut().unwrap().end_of_interrupt();
        }
    }

    pub fn send_init_ipi(&mut self, dest_apic_id: u32) {
        unsafe {
            self.lapic.as_mut().unwrap().send_init_ipi(dest_apic_id);
        }
    }

    pub fn send_sipi(&mut self, vector: u8, dest_apic_id: u32) {
        unsafe {
            self.lapic.as_mut().unwrap().send_sipi(vector, dest_apic_id);
        }
    }
}

#[allow(static_mut_refs)]
pub fn init_lapic(lapic_addr: u64) {
    unsafe {
        LAPIC.init_once(|| Mutex::new(LApic::new(lapic_addr)));
        LAPIC.get().unwrap().lock().init();
    }
}

pub fn init_ioapic(ioapic_addr: u64) {
    IOAPIC.call_once(|| Mutex::new(alloc::vec![IOApic::new(ioapic_addr)]));

    let mut ioapic_lock = IOAPIC.get().unwrap().lock();
    ioapic_lock.push(IOApic::new(ioapic_addr));
}

pub fn init(rsdp_addr: &u64) {
    let handler = crate::arch::x86::acpi::handler::AcpiHandler;
    let tables = unsafe { AcpiTables::from_rsdp(handler, *rsdp_addr as usize).unwrap() };
    let platform = AcpiPlatform::new(tables, handler).unwrap();
    let interrupt_model = platform.interrupt_model;

    debug!("Interrupt Model: {:?}", interrupt_model);

    if let InterruptModel::Apic(apic) = interrupt_model {
        let lapic_physical_address: u64 = apic.local_apic_address;
        init_lapic(lapic_physical_address);
        for i in apic.io_apics.iter() {
            init_ioapic(i.address as u64);
            debug!("IO Pushed: {:?}", i);
        }

        unsafe {
            for ioapic in IOAPIC.get().unwrap().lock().iter_mut() {
                ioapic.init();
                ioapic.enable();
                debug!("IO Enabled: {:?}", ioapic.get_ioapic());
            }
        }

        #[allow(static_mut_refs)]
        unsafe {
            LAPIC.get().unwrap().lock().enable();
        }
    }
}
