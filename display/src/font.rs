use crate::fb::Framebuffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    Regular,
    Italic,
    Bold,
    BoldItalic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphMetric {
    pub offset: usize,
    pub len: usize,
    pub width: u8,
    pub height: u8,
    pub x_offset: i8,
    pub y_offset: i8,
    pub advance: u8,
}

pub struct BitmapFont {
    pub first: u16,
    pub last: u16,
    pub line_height: u8,
    pub baseline: u8,
    pub metrics: &'static [GlyphMetric],
    pub bitmap: &'static [u8],
}

impl BitmapFont {
    pub fn glyph(&self, codepoint: u16) -> Option<(&GlyphMetric, &'static [u8])> {
        if codepoint < self.first || codepoint > self.last {
            return None;
        }
        let index = (codepoint - self.first) as usize;
        let metric = self.metrics.get(index)?;
        Some((
            metric,
            &self.bitmap[metric.offset..metric.offset + metric.len],
        ))
    }
}

pub fn literata(style: FontStyle) -> &'static BitmapFont {
    match style {
        FontStyle::Regular => &crate::literata_generated::LITERATA_REGULAR,
        FontStyle::Italic => &crate::literata_generated::LITERATA_ITALIC,
        FontStyle::Bold => &crate::literata_generated::LITERATA_BOLD,
        FontStyle::BoldItalic => &crate::literata_generated::LITERATA_BOLD_ITALIC,
    }
}

pub fn draw_text(
    fb: &mut Framebuffer,
    font: &BitmapFont,
    text: &str,
    x: i16,
    baseline_y: i16,
    white: bool,
) -> i16 {
    let mut cursor = x;
    for ch in text.chars() {
        let codepoint = ch as u32;
        if codepoint > u16::MAX as u32 {
            continue;
        }
        cursor += draw_glyph(fb, font, codepoint as u16, cursor, baseline_y, white);
    }
    cursor
}

pub fn draw_text_rotated_glyphs(
    fb: &mut Framebuffer,
    font: &BitmapFont,
    text: &str,
    x: i16,
    baseline_y: i16,
    white: bool,
) -> i16 {
    let mut cursor = x;
    for ch in text.chars() {
        let codepoint = ch as u32;
        if codepoint > u16::MAX as u32 {
            continue;
        }
        cursor += draw_glyph_rotated(fb, font, codepoint as u16, cursor, baseline_y, white);
    }
    cursor
}

pub fn draw_text_mirrored_x_glyphs(
    fb: &mut Framebuffer,
    font: &BitmapFont,
    text: &str,
    x: i16,
    baseline_y: i16,
    white: bool,
) -> i16 {
    let mut cursor = x;
    for ch in text.chars() {
        let codepoint = ch as u32;
        if codepoint > u16::MAX as u32 {
            continue;
        }
        cursor += draw_glyph_mirrored_x(fb, font, codepoint as u16, cursor, baseline_y, white);
    }
    cursor
}

pub fn draw_text_mirrored_y_glyphs(
    fb: &mut Framebuffer,
    font: &BitmapFont,
    text: &str,
    x: i16,
    baseline_y: i16,
    white: bool,
) -> i16 {
    let mut cursor = x;
    for ch in text.chars() {
        let codepoint = ch as u32;
        if codepoint > u16::MAX as u32 {
            continue;
        }
        cursor += draw_glyph_mirrored_y(fb, font, codepoint as u16, cursor, baseline_y, white);
    }
    cursor
}

pub fn measure_text(font: &BitmapFont, text: &str) -> u16 {
    text.chars()
        .map(|ch| {
            font.glyph(ch as u16)
                .map(|(metric, _)| metric.advance as u16)
                .unwrap_or(
                    font.glyph(b'?' as u16)
                        .map(|(m, _)| m.advance as u16)
                        .unwrap_or(8),
                )
        })
        .sum()
}

fn draw_glyph(
    fb: &mut Framebuffer,
    font: &BitmapFont,
    codepoint: u16,
    x: i16,
    baseline_y: i16,
    white: bool,
) -> i16 {
    let Some((metric, bitmap)) = font.glyph(codepoint).or_else(|| font.glyph(b'?' as u16)) else {
        return 8;
    };

    let glyph_x = x + metric.x_offset as i16;
    let glyph_y = baseline_y + metric.y_offset as i16;
    let row_bytes = (metric.width as usize).div_ceil(8);
    for y in 0..metric.height as usize {
        for x_byte in 0..row_bytes {
            let byte = bitmap[y * row_bytes + x_byte];
            for bit in 0..8 {
                let px = x_byte * 8 + bit;
                if px >= metric.width as usize {
                    break;
                }
                if byte & (0x80 >> bit) != 0 {
                    let draw_x = glyph_x + px as i16;
                    let draw_y = glyph_y + y as i16;
                    if draw_x >= 0 && draw_y >= 0 {
                        fb.set_pixel(draw_x as usize, draw_y as usize, white);
                    }
                }
            }
        }
    }

    metric.advance as i16
}

fn draw_glyph_rotated(
    fb: &mut Framebuffer,
    font: &BitmapFont,
    codepoint: u16,
    x: i16,
    baseline_y: i16,
    white: bool,
) -> i16 {
    let Some((metric, bitmap)) = font.glyph(codepoint).or_else(|| font.glyph(b'?' as u16)) else {
        return 8;
    };

    let glyph_x = x + metric.x_offset as i16;
    let glyph_y = baseline_y + metric.y_offset as i16;
    let row_bytes = (metric.width as usize).div_ceil(8);
    for y in 0..metric.height as usize {
        for x_byte in 0..row_bytes {
            let byte = bitmap[y * row_bytes + x_byte];
            for bit in 0..8 {
                let px = x_byte * 8 + bit;
                if px >= metric.width as usize {
                    break;
                }
                if byte & (0x80 >> bit) != 0 {
                    let rotated_x = metric.width as usize - 1 - px;
                    let rotated_y = metric.height as usize - 1 - y;
                    let draw_x = glyph_x + rotated_x as i16;
                    let draw_y = glyph_y + rotated_y as i16;
                    if draw_x >= 0 && draw_y >= 0 {
                        fb.set_pixel(draw_x as usize, draw_y as usize, white);
                    }
                }
            }
        }
    }

    metric.advance as i16
}

fn draw_glyph_mirrored_x(
    fb: &mut Framebuffer,
    font: &BitmapFont,
    codepoint: u16,
    x: i16,
    baseline_y: i16,
    white: bool,
) -> i16 {
    let Some((metric, bitmap)) = font.glyph(codepoint).or_else(|| font.glyph(b'?' as u16)) else {
        return 8;
    };

    let glyph_x = x + metric.x_offset as i16;
    let glyph_y = baseline_y + metric.y_offset as i16;
    let row_bytes = (metric.width as usize).div_ceil(8);
    for y in 0..metric.height as usize {
        for x_byte in 0..row_bytes {
            let byte = bitmap[y * row_bytes + x_byte];
            for bit in 0..8 {
                let px = x_byte * 8 + bit;
                if px >= metric.width as usize {
                    break;
                }
                if byte & (0x80 >> bit) != 0 {
                    let mirrored_x = metric.width as usize - 1 - px;
                    let draw_x = glyph_x + mirrored_x as i16;
                    let draw_y = glyph_y + y as i16;
                    if draw_x >= 0 && draw_y >= 0 {
                        fb.set_pixel(draw_x as usize, draw_y as usize, white);
                    }
                }
            }
        }
    }

    metric.advance as i16
}

fn draw_glyph_mirrored_y(
    fb: &mut Framebuffer,
    font: &BitmapFont,
    codepoint: u16,
    x: i16,
    baseline_y: i16,
    white: bool,
) -> i16 {
    let Some((metric, bitmap)) = font.glyph(codepoint).or_else(|| font.glyph(b'?' as u16)) else {
        return 8;
    };

    let glyph_x = x + metric.x_offset as i16;
    let cell_top = baseline_y - font.baseline as i16;
    let row_bytes = (metric.width as usize).div_ceil(8);
    for y in 0..metric.height as usize {
        for x_byte in 0..row_bytes {
            let byte = bitmap[y * row_bytes + x_byte];
            for bit in 0..8 {
                let px = x_byte * 8 + bit;
                if px >= metric.width as usize {
                    break;
                }
                if byte & (0x80 >> bit) != 0 {
                    let source_y = font.baseline as i16 + metric.y_offset as i16 + y as i16;
                    let mirrored_y = font.line_height as i16 - 1 - source_y;
                    let draw_x = glyph_x + px as i16;
                    let draw_y = cell_top + mirrored_y;
                    if draw_x >= 0 && draw_y >= 0 {
                        fb.set_pixel(draw_x as usize, draw_y as usize, white);
                    }
                }
            }
        }
    }

    metric.advance as i16
}
