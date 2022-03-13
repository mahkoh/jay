use bincode::{BorrowDecode, Encode};

#[derive(Encode, BorrowDecode, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub fn set_title_color(color: Color) {
    get!().set_title_color(color)
}

pub fn set_title_underline_color(color: Color) {
    get!().set_title_underline_color(color)
}

pub fn set_border_color(color: Color) {
    get!().set_border_color(color)
}

pub fn set_background_color(color: Color) {
    get!().set_background_color(color)
}

pub fn get_title_height() -> i32 {
    let mut res = 0;
    (|| res = get!().get_title_height())();
    res
}

pub fn get_border_width() -> i32 {
    let mut res = 0;
    (|| res = get!().get_border_width())();
    res
}

pub fn set_title_height(height: i32) {
    get!().set_title_height(height)
}

pub fn set_border_width(width: i32) {
    get!().set_border_width(width)
}
