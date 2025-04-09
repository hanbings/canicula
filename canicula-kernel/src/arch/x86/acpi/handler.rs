use core::ptr::NonNull;

use acpi::PhysicalMapping;
use x86_64::{instructions::port::Port, PhysAddr};

#[derive(Debug, Clone, Copy)]
pub struct AcpiHandler;

impl acpi::AcpiHandler for AcpiHandler {
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct AmlHandler;

impl aml::Handler for AmlHandler {
    fn read_u8(&self, address: usize) -> u8 {
        read_addr::<u8>(address)
    }

    fn read_u16(&self, address: usize) -> u16 {
        read_addr::<u16>(address)
    }

    fn read_u32(&self, address: usize) -> u32 {
        read_addr::<u32>(address)
    }

    fn read_u64(&self, address: usize) -> u64 {
        read_addr::<u64>(address)
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        write_addr::<u8>(address, value)
    }

    fn write_u16(&mut self, address: usize, value: u16) {
        write_addr::<u16>(address, value)
    }

    fn write_u32(&mut self, address: usize, value: u32) {
        write_addr::<u32>(address, value)
    }

    fn write_u64(&mut self, address: usize, value: u64) {
        write_addr::<u64>(address, value)
    }

    // ==== IO Port Read ====
    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { Port::new(port).read() }
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { Port::new(port).read() }
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { Port::new(port).read() }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { Port::new(port).write(value) }
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { Port::new(port).write(value) }
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { Port::new(port).write(value) }
    }

    fn read_pci_u8(&self, seg: u16, bus: u8, dev: u8, func: u8, offset: u16) -> u8 {
        pci_config_read_u32(seg, bus, dev, func, offset) as u8
    }

    fn read_pci_u16(&self, seg: u16, bus: u8, dev: u8, func: u8, offset: u16) -> u16 {
        pci_config_read_u32(seg, bus, dev, func, offset) as u16
    }

    fn read_pci_u32(&self, seg: u16, bus: u8, dev: u8, func: u8, offset: u16) -> u32 {
        pci_config_read_u32(seg, bus, dev, func, offset)
    }

    fn write_pci_u8(&self, seg: u16, bus: u8, dev: u8, func: u8, offset: u16, value: u8) {
        let old = pci_config_read_u32(seg, bus, dev, func, offset);
        let shift = ((offset & 3) * 8) as u32;
        let mask = !(0xFF << shift);
        let new = (old & mask) | ((value as u32) << shift);
        pci_config_write_u32(seg, bus, dev, func, offset, new);
    }

    fn write_pci_u16(&self, seg: u16, bus: u8, dev: u8, func: u8, offset: u16, value: u16) {
        let old = pci_config_read_u32(seg, bus, dev, func, offset);
        let shift = ((offset & 2) * 8) as u32;
        let mask = !(0xFFFF << shift);
        let new = (old & mask) | ((value as u32) << shift);
        pci_config_write_u32(seg, bus, dev, func, offset, new);
    }

    fn write_pci_u32(&self, seg: u16, bus: u8, dev: u8, func: u8, offset: u16, value: u32) {
        pci_config_write_u32(seg, bus, dev, func, offset, value);
    }
}

fn read_addr<T: Copy>(addr: usize) -> T {
    let virt = unsafe {
        crate::arch::x86::memory::physical_to_virtual(PhysAddr::new(addr as u64))
    };
    unsafe { *virt.as_ptr::<T>() }
}

fn write_addr<T: Copy>(addr: usize, value: T) {
    let virt = unsafe {
        crate::arch::x86::memory::physical_to_virtual(PhysAddr::new(addr as u64))
    };
    unsafe { *virt.as_mut_ptr::<T>() = value };
}

fn pci_config_address(bus: u8, dev: u8, func: u8, offset: u16) -> u32 {
    (1 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

fn pci_config_read_u32(_seg: u16, bus: u8, dev: u8, func: u8, offset: u16) -> u32 {
    unsafe {
        let addr = pci_config_address(bus, dev, func, offset);
        Port::new(0xCF8).write(addr);
        Port::new(0xCFC).read()
    }
}

fn pci_config_write_u32(_seg: u16, bus: u8, dev: u8, func: u8, offset: u16, value: u32) {
    unsafe {
        let addr = pci_config_address(bus, dev, func, offset);
        Port::new(0xCF8).write(addr);
        Port::new(0xCFC).write(value);
    }
}
