use log::debug;
use x86_64::instructions::port::Port;

fn pci_class_code_description(class: u8, subclass: u8) -> &'static str {
    match (class, subclass) {
        (0x00, 0x00) => "Unclassified: Non-VGA compatible device",
        (0x00, 0x01) => "Unclassified: VGA compatible device",
        (0x01, 0x00) => "Mass Storage: SCSI",
        (0x01, 0x01) => "Mass Storage: IDE",
        (0x01, 0x06) => "Mass Storage: SATA",
        (0x01, 0x80) => "Mass Storage: Other",
        (0x02, 0x00) => "Network: Ethernet controller",
        (0x03, 0x00) => "Display: VGA compatible controller",
        (0x03, 0x01) => "Display: XGA controller",
        (0x03, 0x80) => "Display: Other",
        (0x06, 0x00) => "Bridge: Host bridge",
        (0x06, 0x01) => "Bridge: ISA bridge",
        (0x06, 0x04) => "Bridge: PCI-to-PCI bridge",
        (0x06, 0x80) => "Bridge: Other bridge device",
        _ => "Unknown device",
    }
}

fn pci_config_read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);

    let mut address_port = Port::<u32>::new(0xCF8);
    let mut data_port = Port::<u32>::new(0xCFC);

    unsafe {
        address_port.write(address);
        data_port.read()
    }
}

pub fn enumerate_pci() {
    for bus in 0u8..=255 {
        for device in 0u8..32 {
            for function in 0u8..8 {
                let vendor_device = pci_config_read(bus, device, function, 0x00);
                if vendor_device == 0xFFFF_FFFF {
                    continue;
                }

                let vendor_id = (vendor_device & 0xFFFF) as u16;
                let device_id = ((vendor_device >> 16) & 0xFFFF) as u16;

                let class_reg = pci_config_read(bus, device, function, 0x08);
                let class_code = ((class_reg >> 24) & 0xFF) as u8;
                let subclass = ((class_reg >> 16) & 0xFF) as u8;
                let prog_if = ((class_reg >> 8) & 0xFF) as u8;

                let header_type_reg = pci_config_read(bus, device, function, 0x0C);
                let header_type = ((header_type_reg >> 16) & 0xFF) as u8;

                debug!(
                    "PCI: Bus {:02X}, Dev {:02X}, Func {:X} => Vendor: {:04X}, Device: {:04X}, Class: {:02X}:{:02X}, ProgIF: {:02X} ({})",
                    bus,
                    device,
                    function,
                    vendor_id,
                    device_id,
                    class_code,
                    subclass,
                    prog_if,
                    pci_class_code_description(class_code, subclass)
                );

                if function == 0 && (header_type & 0x80) == 0 {
                    break;
                }
            }
        }
    }
}

pub fn init() {
    enumerate_pci();
}
