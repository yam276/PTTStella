//! The Rust screen owns glyph pairing and styles. This module only paints that projection.
use super::graphics;
use pttstella::{
    export,
    screen::{Frame, Style},
    settings::DisplaySettings,
    terminal_fonts::{CASCADIA_MONO, NOTO_SANS_CJK_TC},
};
use skia_safe::{
    AlphaType, Color, ColorType, Data, Font, FontMgr, FontStyle, ImageInfo, Paint, Rect, Typeface,
    surfaces,
};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::collections::HashMap;

const CELL_WIDTH: f32 = 12.;
const CELL_HEIGHT: f32 = 24.;
const MAX_EDGE: f32 = 8192.;
const MAX_PIXELS: f32 = (16 * 1024 * 1024) as f32;
const PALETTE: [Color; 16] = [
    Color::from_rgb(0, 0, 0),
    Color::from_rgb(128, 0, 0),
    Color::from_rgb(0, 128, 0),
    Color::from_rgb(128, 128, 0),
    Color::from_rgb(0, 0, 128),
    Color::from_rgb(128, 0, 128),
    Color::from_rgb(0, 128, 128),
    Color::from_rgb(192, 192, 192),
    Color::from_rgb(128, 128, 128),
    Color::from_rgb(255, 0, 0),
    Color::from_rgb(0, 255, 0),
    Color::from_rgb(255, 255, 0),
    Color::from_rgb(0, 0, 255),
    Color::from_rgb(255, 0, 255),
    Color::from_rgb(0, 255, 255),
    Color::from_rgb(255, 255, 255),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn fit(view_width: f32, view_height: f32, dpi: f32, zoom: f32) -> Option<Self> {
        let (w, h) = (export::WIDTH as f32, export::HEIGHT as f32);
        let scale = ((view_width / w).min(view_height / h) * dpi * zoom)
            .min(MAX_EDGE / w.max(h))
            .min((MAX_PIXELS / (w * h)).sqrt());
        if view_width <= 0. || view_height <= 0. || !scale.is_finite() || scale <= 0. {
            return None;
        }
        // Flooring keeps the final allocation within the pixel budget.
        Some(Self {
            width: (w * scale).floor().max(1.) as u32,
            height: (h * scale).floor().max(1.) as u32,
        })
    }

    pub fn cell(self, col: usize, row: usize, span: usize, frame: &Frame) -> Rect {
        let x = |col| edge(col, frame.cols, self.width);
        let y = |row| edge(row, frame.rows, self.height);
        Rect::new(x(col), y(row), x(col + span), y(row + 1))
    }

    pub fn hit(self, x: f32, y: f32, frame: &Frame) -> Option<pttstella::selection::Position> {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.
            || y < 0.
            || x >= self.width as f32
            || y >= self.height as f32
            || frame.cols == 0
            || frame.rows == 0
        {
            return None;
        }
        let col = (1..=frame.cols).find(|&col| x < edge(col, frame.cols, self.width))? - 1;
        let row = (1..=frame.rows).find(|&row| y < edge(row, frame.rows, self.height))? - 1;
        Some(pttstella::selection::Position { col, row })
    }
}

fn edge(index: usize, count: usize, pixels: u32) -> f32 {
    ((index * pixels as usize + count / 2) / count) as f32
}

fn colours(style: Style) -> (Color, Color) {
    let fg = style.fg + if style.bright && style.fg < 8 { 8 } else { 0 };
    let (fg, bg) = (PALETTE[usize::from(fg)], PALETTE[usize::from(style.bg)]);
    if style.inverse { (bg, fg) } else { (fg, bg) }
}

struct Ink {
    font: Font,
    advance: f32,
    ascent: f32,
    descent: f32,
}

impl Ink {
    fn new(font: Font, text: &str, sample: Rect) -> Self {
        let (advance, bounds) = font.measure_str(text, None);
        Self {
            font,
            advance: advance.max(1.),
            ascent: (-bounds.top.min(sample.top)).max(0.),
            descent: bounds.bottom.max(sample.bottom).max(0.),
        }
    }

    fn layout(&self, cell: Rect, span: f32, scale: f32, baseline: f32, width: f32) -> TextLayout {
        let vertical = scale
            .min(span / self.advance)
            .min(cell.height() / (self.ascent + self.descent).max(1.));
        let horizontal = (vertical * width).min(span / self.advance);
        let baseline = (baseline * scale).clamp(
            self.ascent * vertical,
            (cell.height() - self.descent * vertical).max(self.ascent * vertical),
        );
        TextLayout {
            origin: (
                cell.left + (cell.width() - self.advance * horizontal) / 2.,
                cell.top + baseline,
            ),
            scale: (horizontal, vertical),
        }
    }
}

struct TextLayout {
    origin: (f32, f32),
    scale: (f32, f32),
}

pub struct Renderer {
    frame: Frame,
    ink: [HashMap<String, Ink>; 2],
    font_manager: FontMgr,
    faces: [Typeface; 2],
    font_size: f32,
    baseline: f32,
    font_width: f32,
}

impl Renderer {
    pub fn frame(&self) -> &Frame {
        &self.frame
    }
    pub fn new(frame: Frame, display: &DisplaySettings) -> Result<Self, &'static str> {
        let mgr = FontMgr::new();
        let ascii = face(&mgr, &display.ascii_font)
            .or_else(|| face(&mgr, "Cascadia Mono"))
            .ok_or("cannot load the terminal Latin font")?;
        let cjk = face(&mgr, &display.cjk_font)
            .or_else(|| face(&mgr, "Noto Sans CJK TC"))
            .ok_or("cannot load the terminal CJK font")?;
        let mut renderer = Self {
            frame,
            ink: [HashMap::new(), HashMap::new()],
            font_manager: mgr,
            faces: [ascii, cjk],
            font_size: display.font_size as f32,
            baseline: display.baseline as f32,
            font_width: display.font_width as f32 / 100.,
        };
        renderer.cache_glyphs();
        Ok(renderer)
    }

    /// Independent projection state sharing Skia's reference-counted font assets.
    /// Article tiles cannot replace the terminal source used for PNG and input.
    pub fn fork(&self, frame: Frame) -> Self {
        let mut renderer = Self {
            frame,
            ink: [HashMap::new(), HashMap::new()],
            font_manager: self.font_manager.clone(),
            faces: self.faces.clone(),
            font_size: self.font_size,
            baseline: self.baseline,
            font_width: self.font_width,
        };
        renderer.cache_glyphs();
        renderer
    }

    pub fn set_frame(&mut self, frame: Frame) {
        // Keep warmed metrics between redraws, with a cap independent of session length.
        const CACHE_CAPACITY: usize = 4096;
        if self.ink.iter().map(HashMap::len).sum::<usize>() + frame.glyphs.len() > CACHE_CAPACITY {
            self.ink.iter_mut().for_each(HashMap::clear);
        }
        self.frame = frame;
        self.cache_glyphs();
    }

    fn cache_glyphs(&mut self) {
        for glyph in &self.frame.glyphs {
            let cache = &mut self.ink[glyph.width - 1];
            if glyph.text == " "
                || graphic(&glyph.text).is_some()
                || cache.contains_key(&glyph.text)
            {
                continue;
            }
            let primary = &self.faces[glyph.width - 1];
            let supports = |face: &Typeface| {
                glyph
                    .text
                    .chars()
                    .all(|ch| face.unichar_to_glyph(ch as i32) != 0)
            };
            let alternate = &self.faces[2 - glyph.width];
            let fallback = if supports(primary) {
                primary.clone()
            } else if supports(alternate) {
                alternate.clone()
            } else {
                glyph
                    .text
                    .chars()
                    .find(|&ch| primary.unichar_to_glyph(ch as i32) == 0)
                    .and_then(|ch| {
                        self.font_manager.match_family_style_character(
                            "",
                            FontStyle::normal(),
                            &["zh-Hant"],
                            ch as i32,
                        )
                    })
                    .filter(supports)
                    .unwrap_or_else(|| primary.clone())
            };
            let font = Font::from_typeface(fallback, self.font_size);
            let (_, sample) = font.measure_str(
                if glyph.width == 1 {
                    "[]()Agjpqy_"
                } else {
                    "中文"
                },
                None,
            );
            cache.insert(glyph.text.clone(), Ink::new(font, &glyph.text, sample));
        }
    }

    pub fn cursor_rect(&self, size: Size) -> Rect {
        let (col, row) = self.frame.cursor;
        size.cell(col, row, 1, &self.frame)
    }

    pub fn paint(
        &self,
        size: Size,
        cursor: bool,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, &'static str> {
        self.paint_with_find(size, cursor, None)
    }
    pub fn paint_with_find(
        &self,
        size: Size,
        cursor: bool,
        find: Option<&pttstella::article_find::Find>,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, &'static str> {
        let mut pixels = SharedPixelBuffer::new(size.width, size.height);
        let info = ImageInfo::new(
            (size.width as i32, size.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let mut surface = surfaces::wrap_pixels(&info, pixels.make_mut_bytes(), None, None)
            .ok_or("cannot wrap terminal pixel buffer")?;
        let canvas = surface.canvas();
        canvas.clear(Color::BLACK);

        let frame = &self.frame;
        let scale = (size.width as f32 / (frame.cols as f32 * CELL_WIDTH))
            .min(size.height as f32 / (frame.rows as f32 * CELL_HEIGHT));
        let mut paint = Paint::default();
        for glyph in &frame.glyphs {
            let highlight = find.and_then(|f| f.style(glyph.row, glyph.col, glyph.width));
            for half in 0..glyph.width {
                let (_, background) =
                    colours(highlight.unwrap_or(if half == 0 { glyph.left } else { glyph.right }));
                if background != Color::BLACK {
                    paint.set_color(background);
                    canvas.draw_rect(size.cell(glyph.col + half, glyph.row, 1, frame), &paint);
                }
            }
        }

        for glyph in &frame.glyphs {
            if glyph.text == " " {
                continue;
            }
            let cell = size.cell(glyph.col, glyph.row, glyph.width, frame);
            let highlight = find.and_then(|f| f.style(glyph.row, glyph.col, glyph.width));
            let left = colours(highlight.unwrap_or(glyph.left)).0;
            let right = colours(highlight.unwrap_or(glyph.right)).0;
            let split = glyph.width == 2 && left != right;
            let ink = self.ink[glyph.width - 1].get(&glyph.text);
            let text = ink.map(|ink| {
                let span = glyph.width as f32 * CELL_WIDTH * scale;
                (
                    ink,
                    ink.layout(cell, span, scale, self.baseline, self.font_width),
                )
            });
            let graphic = graphic(&glyph.text);
            for half in 0..if split { 2 } else { 1 } {
                canvas.save();
                let clip = if split {
                    size.cell(glyph.col + half, glyph.row, 1, frame)
                } else {
                    cell
                };
                canvas.clip_rect(clip, None, false);
                paint.set_color(if half == 0 { left } else { right });
                if let Some((ink, layout)) = &text {
                    paint.set_anti_alias(true);
                    canvas.translate(layout.origin);
                    canvas.scale(layout.scale);
                    canvas.draw_str(&glyph.text, (0., 0.), &ink.font, &paint);
                    paint.set_anti_alias(false);
                } else if let Some(ch) = graphic {
                    graphics::draw(canvas, ch, cell, &paint);
                }
                canvas.restore();
            }
        }

        if cursor && frame.cursor_visible {
            let cell = self.cursor_rect(size);
            paint.set_color(PALETTE[7]);
            canvas.draw_rect(
                Rect::new(
                    cell.left,
                    cell.bottom - (2. * scale).round().max(1.),
                    cell.right,
                    cell.bottom,
                ),
                &paint,
            );
        }
        drop(surface);
        Ok(pixels)
    }
}

fn graphic(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let ch = chars.next()?;
    (chars.next().is_none() && graphics::is_graphic(ch)).then_some(ch)
}

fn face(mgr: &FontMgr, family: &str) -> Option<Typeface> {
    match family {
        "Cascadia Mono" => mgr.new_from_data(Data::new_copy(CASCADIA_MONO), None),
        "Noto Sans CJK TC" => mgr.new_from_data(Data::new_copy(NOTO_SANS_CJK_TC), None),
        _ => mgr.match_family_style(family, FontStyle::normal()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::screen::Screen;

    #[test]
    fn asymmetric_backgrounds_and_one_glyph_survive_fractional_resize() {
        for (width, height) in [(960, 576), (1201, 720), (1441, 865)] {
            let size = Size { width, height };
            let mut screen = Screen::new(80, 24).unwrap();
            screen.feed(b"\x1b[91;44;66;92;45m\xa4\xa4\x1b[2;1H\x1b[44m \x1b[45m \x1b[0m\x1b[24;79H\x1b[106m  ");
            let frame = screen.frame();
            assert_eq!(frame.glyphs[0].text, "中");
            assert_eq!(frame.glyphs[0].width, 2);
            let display = pttstella::replay::parity_display_profile()
                .resolve()
                .unwrap();
            let renderer = Renderer::new(frame, &display).unwrap();
            let pixels = renderer.paint(size, false).unwrap();
            let pixel = |x: usize, y: usize| pixels.as_slice()[y * width as usize + x];
            let split = (width as f32 / 80.).round() as usize;
            let end = (width as f32 / 40.).round() as usize;
            let next_row = (height as f32 / 24.).round() as usize;
            assert_eq!(
                pixel(split - 1, next_row),
                Rgba8Pixel {
                    r: 0,
                    g: 0,
                    b: 128,
                    a: 255
                }
            );
            assert_eq!(
                pixel(split, next_row),
                Rgba8Pixel {
                    r: 128,
                    g: 0,
                    b: 128,
                    a: 255
                }
            );
            assert_eq!(
                pixel(width as usize - 1, height as usize - 1),
                Rgba8Pixel {
                    r: 0,
                    g: 255,
                    b: 255,
                    a: 255
                }
            );
            let mut has_red_ink = false;
            let mut has_green_ink = false;
            for y in 0..next_row {
                for x in 0..split {
                    has_red_ink |= pixel(x, y).r > 128 && pixel(x, y).g == 0;
                }
                for x in split..end {
                    has_green_ink |= pixel(x, y).g > 128;
                }
            }
            assert!(has_red_ink && has_green_ink);
        }
    }

    #[test]
    fn article_projection_shares_faces_but_keeps_terminal_source_and_pixels() {
        let display = pttstella::replay::parity_display_profile()
            .resolve()
            .unwrap();
        let mut screen = pttstella::screen::Screen::new(80, 24).unwrap();
        screen.feed(&pttstella::replay::parity_grid().unwrap());
        let frame = screen.frame();
        let renderer = Renderer::new(frame.clone(), &display).unwrap();
        let size = Size {
            width: 960,
            height: 576,
        };
        let expected = renderer.paint(size, false).unwrap();
        let mut row = pttstella::screen::Screen::new(80, 1).unwrap();
        row.feed(b"\x1b[31;44mArticle projection\x1b[K");
        let article = renderer.fork(row.frame());
        assert_eq!(renderer.frame(), &frame);
        assert_eq!(article.frame().rows, 1);
        let pixels = article
            .paint(
                Size {
                    width: 960,
                    height: 24,
                },
                false,
            )
            .unwrap();
        assert_eq!(pixels.as_slice()[959].b, 128);
        assert_eq!(
            renderer.paint(size, false).unwrap().as_bytes(),
            expected.as_bytes()
        );
    }

    #[test]
    fn fit_uses_physical_pixels_and_bounds_zoom() {
        assert_eq!(
            Size::fit(960., 600., 1.25, 1.),
            Some(Size {
                width: 1200,
                height: 720
            })
        );
        assert_eq!(
            Size::fit(960., 600., 2., 1.5),
            Some(Size {
                width: 2880,
                height: 1728
            })
        );
        let large = Size::fit(100_000., 100_000., 2., 1.5).unwrap();
        assert!(large.width * large.height <= 16 * 1024 * 1024);
        assert!(large.width <= 8192 && large.height <= 8192);
        assert_eq!(Size::fit(0., 600., 1., 1.), None);
    }
}
