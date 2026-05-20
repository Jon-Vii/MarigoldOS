use crate::{FB_BYTES, HEIGHT, ROW_BYTES, WIDTH};

pub struct Framebuffer {
    data: [u8; FB_BYTES],
}

impl Framebuffer {
    pub const fn new() -> Self {
        Self {
            data: [0xFF; FB_BYTES],
        }
    }

    #[inline]
    pub fn bytes(&self) -> &[u8; FB_BYTES] {
        &self.data
    }

    pub fn clear(&mut self, white: bool) {
        self.data.fill(if white { 0xFF } else { 0x00 });
    }

    pub fn copy_from(&mut self, other: &Self) {
        self.data.copy_from_slice(other.bytes());
    }

    pub fn band(&self, y: usize, rows: usize) -> &[u8] {
        let start = y * ROW_BYTES;
        let end = start + rows.min(HEIGHT - y) * ROW_BYTES;
        &self.data[start..end]
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, white: bool) {
        if x >= WIDTH || y >= HEIGHT {
            return;
        }

        let index = y * ROW_BYTES + x / 8;
        let mask = 0x80 >> (x & 7);
        if white {
            self.data[index] |= mask;
        } else {
            self.data[index] &= !mask;
        }
    }

    #[inline]
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        if x >= WIDTH || y >= HEIGHT {
            return true;
        }

        let index = y * ROW_BYTES + x / 8;
        let mask = 0x80 >> (x & 7);
        self.data[index] & mask != 0
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}
