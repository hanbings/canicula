use core::fmt::Write;

use canicula_common::entry::FrameBuffer;

use lazy_static::lazy_static;
use noto_sans_mono_bitmap::FontWeight;
use noto_sans_mono_bitmap::RasterHeight;
use noto_sans_mono_bitmap::get_raster;
use spin::Mutex;
use x86_64::instructions::interrupts;

lazy_static! {
    pub static ref CONSOLE: Mutex<Option<NotoFontDisplay>> = Mutex::new(None);
}

pub struct NotoFontDisplay {
    width: usize,
    height: usize,
    // pixels per row in memory (may include alignment padding)
    stride: usize,
    draw_buffer: &'static mut [u32],

    font_weight: FontWeight,
    raster_height: RasterHeight,

    cursor_x: usize,
    cursor_y: usize,
}

pub fn init(frame_buffer: &mut FrameBuffer) {
    let buffer = frame_buffer.buffer_mut().as_ptr() as *mut u32;
    let width = frame_buffer.info().width;
    let height = frame_buffer.info().height;
    // use stride instead of width for memory calculation
    let stride = frame_buffer.info().stride;

    // use stride * height as buffer size
    for index in 0..(stride * height) {
        unsafe {
            buffer.add(index as usize).write(0xff408deb);
        }
    }

    let console = NotoFontDisplay::new(
        width as usize,
        height as usize,
        stride as usize,
        unsafe { core::slice::from_raw_parts_mut(buffer, (stride * height) as usize) },
        FontWeight::Light,
        RasterHeight::Size24,
    );

    interrupts::without_interrupts(|| {
        CONSOLE.lock().replace(console);

        CONSOLE
            .lock()
            .as_mut()
            .unwrap()
            .draw_string("Kernel Message");
    });
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    interrupts::without_interrupts(|| {
        CONSOLE
            .lock()
            .as_mut()
            .unwrap()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::arch::x86::console::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
        concat!($fmt, "\n"), $($arg)*));
}

impl NotoFontDisplay {
    pub fn new(
        width: usize,
        height: usize,
        stride: usize,
        draw_buffer: &'static mut [u32],
        font_weight: FontWeight,
        raster_height: RasterHeight,
    ) -> Self {
        Self {
            width,
            height,
            stride,
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
        for (char_i, char) in msg.chars().enumerate() {
            let char_raster = match get_raster(char, self.font_weight, self.raster_height) {
                Some(raster) => raster,
                None => get_raster(' ', self.font_weight, self.raster_height).unwrap(),
            };
            for (row_i, row) in char_raster.raster().iter().enumerate() {
                for (col_i, intensity) in row.iter().enumerate() {
                    // use stride for row offset calculation instead of width
                    let index = char_i * char_raster.width()
                        + col_i
                        + row_i * self.stride
                        + (self.cursor_x as usize)
                        + (self.cursor_y as usize * self.stride);

                    let curr_pixel_rgb = self.draw_buffer[index];
                    let mut r = ((curr_pixel_rgb & 0xff0000) >> 16) as u8;
                    let mut g = ((curr_pixel_rgb & 0xff00) >> 8) as u8;
                    let mut b = (curr_pixel_rgb & 0xff) as u8;

                    r = r.saturating_add(*intensity);
                    g = g.saturating_add(*intensity);
                    b = b.saturating_add(*intensity);

                    let new_pixel_rgb = ((r as u32) << 16) + ((g as u32) << 8) + (b as u32);
                    self.draw_buffer[index] = new_pixel_rgb;
                }
            }
        }

        self.cursor_y += msg.len();

        // use width for visible area boundary check
        if self.cursor_x >= self.width {
            self.cursor_y = 0;
            self.cursor_x += self.raster_height as usize;
        }
    }
}

impl Write for NotoFontDisplay {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.draw_string(s);
        Ok(())
    }
}
