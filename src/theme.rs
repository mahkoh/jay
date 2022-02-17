use std::cell::Cell;
use crate::NumCell;

#[derive(Copy, Clone, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

fn to_f32(c: u8) -> f32 {
    c as f32 / 255 as f32
}

impl Color {
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: to_f32(r),
            g: to_f32(g),
            b: to_f32(b),
            a: to_f32(a),
        }
    }
}

impl From<i4config::theme::Color> for Color {
    fn from(f: i4config::theme::Color) -> Self {
        Self {
            r: to_f32(f.r),
            g: to_f32(f.g),
            b: to_f32(f.b),
            a: to_f32(f.a),
        }
    }
}

pub struct Theme {
    pub background_color: Cell<Color>,
    pub title_color: Cell<Color>,
    pub underline_color: Cell<Color>,
    pub border_color: Cell<Color>,
    pub title_height: Cell<i32>,
    pub border_width: Cell<i32>,
    pub version: NumCell<u32>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background_color: Cell::new(Color::from_rgba(0, 0, 0, 255)),
            title_color: Cell::new(Color::from_rgba(0x46, 0x04, 0x17, 255)),
            underline_color: Cell::new(Color::from_rgba(0x66, 0x24, 0x37, 255)),
            border_color: Cell::new(Color::from_rgba(0x36, 0x00, 0x07, 255)),
            title_height: Cell::new(17),
            border_width: Cell::new(4),
            version: NumCell::new(1),
        }
    }
}
