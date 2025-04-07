#![allow(dead_code)]

use x86_64::instructions::port::Port;

const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;

const VBE_DISPI_INDEX_ID: u16 = 0x00;
const VBE_DISPI_INDEX_XRES: u16 = 0x01;
const VBE_DISPI_INDEX_YRES: u16 = 0x02;
const VBE_DISPI_INDEX_BPP: u16 = 0x03;
const VBE_DISPI_INDEX_ENABLE: u16 = 0x04;
const VBE_DISPI_INDEX_BANK: u16 = 0x05;
const VBE_DISPI_INDEX_VIRT_WIDTH: u16 = 0x06;
const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 0x07;
const VBE_DISPI_INDEX_X_OFFSET: u16 = 0x08;
const VBE_DISPI_INDEX_Y_OFFSET: u16 = 0x09;

const VBE_DISPI_ID5: u16 = 0xB0C5;
const VBE_DISPI_DISABLED: u16 = 0x00;
const VBE_DISPI_ENABLED: u16 = 0x01;
const VBE_DISPI_LFB_ENABLED: u16 = 0x02;
const VBE_DISPI_NOCLEARMEM: u16 = 0x04;

pub const VBE_DISPI_BPP_4: u16 = 0x04;
pub const VBE_DISPI_BPP_8: u16 = 0x08;
pub const VBE_DISPI_BPP_15: u16 = 0x0F;
pub const VBE_DISPI_BPP_16: u16 = 0x10;
pub const VBE_DISPI_BPP_24: u16 = 0x18;
pub const VBE_DISPI_BPP_32: u16 = 0x20;

pub fn bga_write_register(index_value: u16, data_value: u16) {
    let mut index_port = Port::<u16>::new(VBE_DISPI_IOPORT_INDEX);
    let mut data_port = Port::<u16>::new(VBE_DISPI_IOPORT_DATA);
    unsafe {
        index_port.write(index_value);
        data_port.write(data_value);
    }
}

pub fn bga_read_register(index_value: u16) -> u16 {
    let mut index_port = Port::<u16>::new(VBE_DISPI_IOPORT_INDEX);
    let mut data_port = Port::<u16>::new(VBE_DISPI_IOPORT_DATA);
    unsafe {
        index_port.write(index_value);
        data_port.read()
    }
}

pub fn bga_is_available() -> bool {
    bga_read_register(VBE_DISPI_INDEX_ID) == VBE_DISPI_ID5
}

pub fn bga_set_video_mode(
    width: u32,
    height: u32,
    bit_depth: u32,
    use_linear_frame_buffer: bool,
    clear_video_memory: bool,
) {
    bga_write_register(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);

    bga_write_register(VBE_DISPI_INDEX_XRES, width as u16);
    bga_write_register(VBE_DISPI_INDEX_YRES, height as u16);
    bga_write_register(VBE_DISPI_INDEX_BPP, bit_depth as u16);

    let mut enable_value = VBE_DISPI_ENABLED;
    if use_linear_frame_buffer {
        enable_value |= VBE_DISPI_LFB_ENABLED;
    }
    if !clear_video_memory {
        enable_value |= VBE_DISPI_NOCLEARMEM;
    }
    bga_write_register(VBE_DISPI_INDEX_ENABLE, enable_value);
}

pub fn bga_set_bank(bank_number: u16) {
    bga_write_register(VBE_DISPI_INDEX_BANK, bank_number);
}
