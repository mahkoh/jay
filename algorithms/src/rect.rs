pub mod region;

use {
    smallvec::SmallVec,
    std::fmt::{Debug, Formatter},
};

#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub struct RectRaw {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Debug for RectRaw {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rect")
            .field("x1", &self.x1)
            .field("y1", &self.y1)
            .field("x2", &self.x2)
            .field("y2", &self.y2)
            .field("width", &(self.x2 - self.x1))
            .field("height", &(self.y2 - self.y1))
            .finish()
    }
}

impl RectRaw {
    fn is_empty(&self) -> bool {
        self.x1 == self.x2 || self.y1 == self.y2
    }
}

type Container = SmallVec<[RectRaw; 1]>;
