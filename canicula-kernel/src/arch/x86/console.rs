use core::fmt;

use lazy_static::lazy_static;
use noto_sans_mono_bitmap::{get_raster, FontWeight, RasterHeight};
use spin::Mutex;

lazy_static! {
    static ref CONSOLE: Mutex<Option<NotoFontDisplay>> = {
        Mutex::new(None)
    };
}

pub fn init(width: usize, height: usize, draw_buffer: &'static mut [u32], font_weight: FontWeight, raster_height: RasterHeight) {
    let console = NotoFontDisplay::new(width, height, draw_buffer, font_weight, raster_height);
    CONSOLE.lock().replace(console);
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    CONSOLE
        .lock()
        .as_mut()
        .unwrap()
        .write_fmt(args)
        .expect("Printing to console failed");
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::arch::x86::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub struct NotoFontDisplay {
    width: usize,
    height: usize,
    draw_buffer: &'static mut [u32],

    font_weight: FontWeight,
    raster_height: RasterHeight,

    cursor_x: usize,
    cursor_y: usize,
}

impl NotoFontDisplay {
    pub fn new(
        width: usize,
        height: usize,
        draw_buffer: &'static mut [u32],
        font_weight: FontWeight,
        raster_height: RasterHeight,
    ) -> Self {
        Self {
            width,
            height,
            draw_buffer,

            font_weight,
            raster_height,

            cursor_x: 0,
            cursor_y: 0,
        }
    }

    pub fn clear(&mut self) {
        for pixel in self.draw_buffer.iter_mut() {
            *pixel = 0;
        }
    }

    pub fn draw_string(&mut self, msg: &str) {
        draw_string(
            msg,
            self.cursor_x as u32,
            self.cursor_y as u32,
            self.width,
            self.height,
            self.font_weight,
            self.raster_height,
            self.draw_buffer,
        );

        self.cursor_y += msg.len();

        if self.cursor_x >= self.width {
            self.cursor_y = 0;
            self.cursor_x += self.raster_height as usize;
        }
    }
}

impl fmt::Write for NotoFontDisplay {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.draw_string(s);
        Ok(())
    }
}

pub fn draw_string(
    msg: &str,
    x: u32,
    y: u32,
    width: usize,
    _height: usize,
    font_weight: FontWeight,
    raster_height: RasterHeight,
    draw_buffer: &mut [u32],
) {
    for (char_i, char) in msg.chars().enumerate() {
        let char_raster = get_raster(char, font_weight, raster_height).expect("unknown char");
        for (row_i, row) in char_raster.raster().iter().enumerate() {
            for (col_i, intensity) in row.iter().enumerate() {
                let index = char_i * char_raster.width()
                    + col_i
                    + row_i * width
                    + (x as usize)
                    + (y as usize * width);

                let curr_pixel_rgb = draw_buffer[index];
                let mut r = ((curr_pixel_rgb & 0xff0000) >> 16) as u8;
                let mut g = ((curr_pixel_rgb & 0xff00) >> 8) as u8;
                let mut b = (curr_pixel_rgb & 0xff) as u8;

                r = r.saturating_add(*intensity);
                g = g.saturating_add(*intensity);
                b = b.saturating_add(*intensity);

                let new_pixel_rgb = ((r as u32) << 16) + ((g as u32) << 8) + (b as u32);

                draw_buffer[index] = new_pixel_rgb;
            }
        }
    }
}
