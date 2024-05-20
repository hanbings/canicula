#![no_std]
#![no_main]

use uefi::{
    prelude::*,
    proto::console::gop::{BltOp, BltPixel, GraphicsOutput},
};

#[entry]
fn main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init(&mut system_table).unwrap();

    // load boot table
    let boot_table = system_table.boot_services();

    // load graphics driver
    let gop_handle = boot_table
        .get_handle_for_protocol::<GraphicsOutput>()
        .unwrap();

    let mut gop = boot_table
        .open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .unwrap();

    // enumerate modes
    let mode = gop.query_mode(12, &boot_table).unwrap();
    let _ = gop.set_mode(&mode);

    // fill screen
    let red_blt_pixel = BltPixel::new(255, 221, 131);
    let (width, _height) = mode.info().resolution();

    let op = BltOp::VideoFill {
        color: red_blt_pixel,
        dest: (0, 0),
        dims: (width, 80),
    };
    let _ = gop.blt(op);

    boot_table.stall(10_000_000);
    Status::SUCCESS
}
