use {
    crate::{theme::Color, utils::errorfmt::ErrorFmt},
    std::ops::Range,
};

pub fn parse_color(string: &str) -> Color {
    let hex = match string.strip_prefix("#") {
        Some(s) => s,
        _ => fatal!("Color must start with #"),
    };
    let d = |range: Range<usize>| match u8::from_str_radix(&hex[range.clone()], 16) {
        Ok(n) => n,
        Err(e) => {
            fatal!(
                "Could not parse color component {}: {}",
                &hex[range],
                ErrorFmt(e)
            )
        }
    };
    let s = |range: Range<usize>| {
        let v = d(range);
        (v << 4) | v
    };
    let (r, g, b, a) = match hex.len() {
        3 => (s(0..1), s(1..2), s(2..3), u8::MAX),
        4 => (s(0..1), s(1..2), s(2..3), s(3..4)),
        6 => (d(0..2), d(2..4), d(4..6), u8::MAX),
        8 => (d(0..2), d(2..4), d(4..6), d(6..8)),
        _ => fatal!(
            "Unexpected length of color string (should be 3, 4, 6, or 8): {}",
            hex.len()
        ),
    };
    jay_config::theme::Color::new_straight(r, g, b, a).into()
}
