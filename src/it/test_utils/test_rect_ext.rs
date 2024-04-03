use crate::rect::Rect;

pub trait TestRectExt {
    fn center(&self) -> (i32, i32);
}

impl TestRectExt for Rect {
    fn center(&self) -> (i32, i32) {
        ((self.x1() + self.x2()) / 2, (self.y1() + self.y2()) / 2)
    }
}
