#![no_std]
#![no_main]

use canicula_common::fs::OperateError;
use types::super_block::SuperBlock;

mod types;

#[allow(unused)]
struct Ext4FS<const SIZE: usize> {
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
        Ext4FS {
            read_byte,
            write_byte,
            super_block: None,
        }
    }

    pub fn init() {}
}
