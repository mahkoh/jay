mod region;

#[cfg(test)]
mod tests;

pub use region::RegionBuilder;
use {
    algorithms::rect::RectRaw,
    smallvec::SmallVec,
    std::fmt::{Debug, Formatter},
};

#[derive(Copy, Clone, Eq, PartialEq, Default)]
#[repr(transparent)]
pub struct Rect {
    raw: RectRaw,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Region {
    rects: SmallVec<[RectRaw; 1]>,
    extents: Rect,
}

impl Debug for Rect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.raw, f)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct RectOverflow {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl RectOverflow {
    pub fn is_contained(&self) -> bool {
        self.left <= 0 && self.right <= 0 && self.top <= 0 && self.bottom <= 0
    }

    pub fn x_overflow(&self) -> bool {
        self.left > 0 || self.right > 0
    }

    pub fn y_overflow(&self) -> bool {
        self.top > 0 || self.bottom > 0
    }
}

impl Rect {
    #[allow(dead_code)]
    pub fn new_empty(x: i32, y: i32) -> Self {
        Self {
            raw: RectRaw {
                x1: x,
                y1: y,
                x2: x,
                y2: y,
            },
        }
    }

    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Option<Self> {
        if x2 < x1 || y2 < y1 {
            return None;
        }
        Some(Self {
            raw: RectRaw { x1, y1, x2, y2 },
        })
    }

    #[allow(dead_code)]
    fn new_unchecked(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            raw: RectRaw { x1, y1, x2, y2 },
        }
    }

    pub fn new_sized(x1: i32, y1: i32, width: i32, height: i32) -> Option<Self> {
        if width < 0 || height < 0 {
            return None;
        }
        Self::new(x1, y1, x1 + width, y1 + height)
    }

    pub fn union(&self, other: Self) -> Self {
        Self {
            raw: RectRaw {
                x1: self.raw.x1.min(other.raw.x1),
                y1: self.raw.y1.min(other.raw.y1),
                x2: self.raw.x2.max(other.raw.x2),
                y2: self.raw.y2.max(other.raw.y2),
            },
        }
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.raw.x1 < other.raw.x2
            && other.raw.x1 < self.raw.x2
            && self.raw.y1 < other.raw.y2
            && other.raw.y1 < self.raw.y2
    }

    pub fn intersect(&self, other: Self) -> Self {
        let x1 = self.raw.x1.max(other.raw.x1);
        let y1 = self.raw.y1.max(other.raw.y1);
        let x2 = self.raw.x2.min(other.raw.x2).max(x1);
        let y2 = self.raw.y2.min(other.raw.y2).max(y1);
        Self {
            raw: RectRaw { x1, y1, x2, y2 },
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.raw.x1 <= x && self.raw.y1 <= y && self.raw.x2 > x && self.raw.y2 > y
    }

    #[allow(dead_code)]
    pub fn contains_rect(&self, rect: &Self) -> bool {
        self.raw.x1 <= rect.raw.x1
            && self.raw.y1 <= rect.raw.x1
            && rect.raw.x2 <= self.raw.x2
            && rect.raw.y2 <= self.raw.y2
    }

    pub fn get_overflow(&self, child: &Self) -> RectOverflow {
        RectOverflow {
            left: self.raw.x1 - child.raw.x1,
            right: child.raw.x2 - self.raw.x2,
            top: self.raw.y1 - child.raw.y1,
            bottom: child.raw.y2 - self.raw.y2,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.raw.x1 == self.raw.x2 || self.raw.y1 == self.raw.y2
    }

    #[allow(dead_code)]
    pub fn to_origin(&self) -> Self {
        Self {
            raw: RectRaw {
                x1: 0,
                y1: 0,
                x2: self.raw.x2 - self.raw.x1,
                y2: self.raw.y2 - self.raw.y1,
            },
        }
    }

    pub fn move_(&self, dx: i32, dy: i32) -> Self {
        Self {
            raw: RectRaw {
                x1: self.raw.x1.saturating_add(dx),
                y1: self.raw.y1.saturating_add(dy),
                x2: self.raw.x2.saturating_add(dx),
                y2: self.raw.y2.saturating_add(dy),
            },
        }
    }

    pub fn at_point(&self, x1: i32, y1: i32) -> Self {
        Self {
            raw: RectRaw {
                x1,
                y1,
                x2: x1 + self.raw.x2 - self.raw.x1,
                y2: y1 + self.raw.y2 - self.raw.y1,
            },
        }
    }

    pub fn with_size(&self, width: i32, height: i32) -> Option<Self> {
        Self::new_sized(self.raw.x1, self.raw.y1, width, height)
    }

    pub fn translate(&self, x: i32, y: i32) -> (i32, i32) {
        (x.wrapping_sub(self.raw.x1), y.wrapping_sub(self.raw.y1))
    }

    pub fn translate_inv(&self, x: i32, y: i32) -> (i32, i32) {
        (x.wrapping_add(self.raw.x1), y.wrapping_add(self.raw.y1))
    }

    pub fn x1(&self) -> i32 {
        self.raw.x1
    }

    pub fn x2(&self) -> i32 {
        self.raw.x2
    }

    pub fn y1(&self) -> i32 {
        self.raw.y1
    }

    pub fn y2(&self) -> i32 {
        self.raw.y2
    }

    pub fn width(&self) -> i32 {
        self.raw.x2 - self.raw.x1
    }

    pub fn height(&self) -> i32 {
        self.raw.y2 - self.raw.y1
    }

    pub fn position(&self) -> (i32, i32) {
        (self.raw.x1, self.raw.y1)
    }

    pub fn size(&self) -> (i32, i32) {
        (self.width(), self.height())
    }
}
