use {
    crate::rect::Rect,
    serde::{Deserialize, Serialize},
};

pub mod sm_wire_session;
pub mod sm_wire_toplevel;

#[derive(Copy, Clone, Serialize, Deserialize)]
struct WireRect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl From<Rect> for WireRect {
    fn from(value: Rect) -> Self {
        Self {
            x1: value.x1(),
            y1: value.y1(),
            x2: value.x2(),
            y2: value.y2(),
        }
    }
}

impl From<WireRect> for Rect {
    fn from(value: WireRect) -> Self {
        Self::new_saturating(value.x1, value.y1, value.x2, value.y2)
    }
}
