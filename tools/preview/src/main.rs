use display::fb::Framebuffer;
use display::font::{draw_text, literata, FontStyle};
use display::render::{draw_ascii, fill_rect, stroke_rect};
use display::{Rect, HEIGHT, WIDTH};
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() -> std::io::Result<()> {
    let out = Path::new("target/previews");
    create_dir_all(out)?;
    write_home(&out.join("home.pbm"))?;
    write_files(&out.join("files.pbm"))?;
    write_reading(&out.join("reading.pbm"))?;
    write_chapters(&out.join("chapters.pbm"))?;
    write_settings(&out.join("settings.pbm"))?;
    Ok(())
}

fn write_home(path: &Path) -> std::io::Result<()> {
    let mut fb = Framebuffer::new();
    stroke_rect(&mut fb, Rect::new(0, 0, WIDTH as u16, HEIGHT as u16), false);
    draw_ascii(&mut fb, "XTEINK", 32, 28, false);
    stroke_rect(&mut fb, Rect::new(228, 86, 344, 274), false);
    draw_ascii(&mut fb, "Flowers for Algernon", 312, 390, false);
    draw_ascii(&mut fb, "Daniel Keyes", 352, 420, false);
    write_pbm(path, &fb)
}

fn write_files(path: &Path) -> std::io::Result<()> {
    let mut fb = Framebuffer::new();
    draw_ascii(&mut fb, "FILES", 64, 72, false);
    fill_rect(&mut fb, Rect::new(64, 110, 352, 2), false);
    draw_ascii(&mut fb, "> Flowers for Algernon", 76, 198, false);
    draw_ascii(&mut fb, "  Daniel Keyes", 112, 230, false);
    write_pbm(path, &fb)
}

fn write_reading(path: &Path) -> std::io::Result<()> {
    let mut fb = Framebuffer::new();
    let heading = literata(FontStyle::Bold);
    let body = literata(FontStyle::Regular);
    draw_text(&mut fb, heading, "Chapter 1", 320, 54, false);
    fill_rect(&mut fb, Rect::new(32, 76, 736, 2), false);
    let mut y = 120;
    for line in [
        "This is the first text-only EPUB reader surface.",
        "It uses generated Literata bitmap glyphs and",
        "keeps pagination as bounded data instead of a DOM.",
    ] {
        draw_text(&mut fb, body, line, 72, y, false);
        y += body.line_height as i16;
    }
    fill_rect(&mut fb, Rect::new(32, 424, 736, 2), false);
    draw_ascii(&mut fb, "Flowers for Algernon", 32, 444, false);
    write_pbm(path, &fb)
}

fn write_chapters(path: &Path) -> std::io::Result<()> {
    let mut fb = Framebuffer::new();
    draw_ascii(&mut fb, "CHAPTERS", 96, 112, false);
    for (index, item) in ["Chapter 1", "Chapter 2", "Chapter 3"].iter().enumerate() {
        draw_ascii(&mut fb, item, 136, 168 + index * 44, false);
    }
    write_pbm(path, &fb)
}

fn write_settings(path: &Path) -> std::io::Result<()> {
    let mut fb = Framebuffer::new();
    draw_ascii(&mut fb, "SETTINGS", 64, 72, false);
    fill_rect(&mut fb, Rect::new(64, 110, 352, 2), false);
    draw_ascii(&mut fb, "> ORIENTATION", 76, 172, false);
    draw_ascii(&mut fb, "  REFRESH", 76, 220, false);
    write_pbm(path, &fb)
}

fn write_pbm(path: &Path, fb: &Framebuffer) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    writeln!(file, "P1")?;
    writeln!(file, "{} {}", WIDTH, HEIGHT)?;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let black = !fb.pixel(x, y);
            write!(file, "{} ", black as u8)?;
        }
        writeln!(file)?;
    }
    Ok(())
}
