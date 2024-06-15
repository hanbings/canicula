#![cfg_attr(not(test), no_std)]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use canicula_common::fs::OperateError;
use core::mem::MaybeUninit;
use types::super_block::SuperBlock;

mod tests;
mod types;

const GROUP_ZERO_PADDING: usize = 1024;

#[allow(unused)]
pub struct Ext4FS<const SIZE: usize> {
    read_byte: fn(usize) -> Result<u8, OperateError>,
    write_byte: fn(u8, usize) -> Result<usize, OperateError>,
    super_block: Option<SuperBlock>,
}

#[allow(unused)]
impl<const SIZE: usize> Ext4FS<SIZE> {
    pub fn new(
        read_byte: fn(usize) -> Result<u8, OperateError>,
        write_byte: fn(u8, usize) -> Result<usize, OperateError>,
    ) -> Self {
        let mut super_block = MaybeUninit::<SuperBlock>::uninit();
        let void_super_block_fields = unsafe {
            core::slice::from_raw_parts(
                &super_block as *const _ as *const u8,
                core::mem::size_of::<SuperBlock>(),
            )
        };

        let mut data_index = 0;
        for (i, field) in void_super_block_fields
            .chunks(core::mem::size_of::<u8>())
            .enumerate()
        {
            // get the current field length.
            let size = field.len();
            let length = size / 8;
            let mut contents: Vec<u8> = Vec::new();

            // read data from physical device.
            let mut count = 0;
            while count < length {
                count = count + 1;
                let byte = (read_byte)(length + GROUP_ZERO_PADDING);

                match byte {
                    Ok(byte) => contents.push(byte),
                    Err(_) => contents.push(0),
                }
            }

            let ptr = super_block.as_mut_ptr();
            // initialize the value of the structure
            // because it is little-endian data, it is read backwards
            for content in contents.iter().rev() {
                unsafe {
                    core::ptr::write((ptr as *const u8).offset(data_index) as *mut u8, *content)
                };
                data_index = data_index + 1;
            }
        }

        Ext4FS {
            read_byte,
            write_byte,
            super_block: Some(unsafe { super_block.assume_init() }),
        }
    }
}
