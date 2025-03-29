use bootloader_api::info::FrameBuffer;

use noto_sans_mono_bitmap::get_raster;
use noto_sans_mono_bitmap::FontWeight;
use noto_sans_mono_bitmap::RasterHeight;

pub struct NotoFontDisplay {
    width: usize,
    height: usize,
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

    for index in 0..(width * height) {
        unsafe {
            buffer.add(index as usize).write(0xff408deb);
        }
    }

    let mut console = NotoFontDisplay::new(
        width as usize,
        height as usize,
        unsafe { core::slice::from_raw_parts_mut(buffer, (width * height) as usize) },
        FontWeight::Light,
        RasterHeight::Size16,
    );

    console.draw_string("Kernel Message");
    console.draw_string("Kernel Second Message");
}

#[allow(dead_code)]
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
