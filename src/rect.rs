use std::fmt::{Debug, Formatter};

#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Debug for Rect {
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
    pub fn new_empty(x: i32, y: i32) -> Self {
        Self {
            x1: x,
            y1: y,
            x2: x,
            y2: y,
        }
    }

    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Option<Self> {
        if x2 < x1 || y2 < y1 {
            return None;
        }
        Some(Self { x1, y1, x2, y2 })
    }

    pub fn new_sized(x1: i32, y1: i32, width: i32, height: i32) -> Option<Self> {
        if width < 0 || height < 0 {
            return None;
        }
        Self::new(x1, y1, x1 + width, y1 + height)
    }

    pub fn union(&self, other: Self) -> Self {
        Self {
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
            x2: self.x2.max(other.x2),
            y2: self.y2.max(other.y2),
        }
    }

    #[allow(dead_code)]
    pub fn intersects(&self, other: &Self) -> bool {
        let x1 = self.x1.max(other.x1);
        let y1 = self.y1.max(other.y1);
        let x2 = self.x2.min(other.x2);
        let y2 = self.y2.min(other.y2);
        x1 < x2 && y1 < y2
    }

    pub fn intersect(&self, other: Self) -> Self {
        let x1 = self.x1.max(other.x1);
        let y1 = self.y1.max(other.y1);
        let x2 = self.x2.min(other.x2).max(x1);
        let y2 = self.y2.min(other.y2).max(y1);
        Self { x1, y1, x2, y2 }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.x1 <= x && self.y1 <= y && self.x2 > x && self.y2 > y
    }

    pub fn contains_rect(&self, rect: &Self) -> bool {
        self.x1 <= rect.x1 && self.y1 <= rect.x1 && rect.x2 <= self.x2 && rect.y2 <= self.y2
    }

    pub fn get_overflow(&self, child: &Self) -> RectOverflow {
        RectOverflow {
            left: self.x1 - child.x1,
            right: child.x2 - self.x2,
            top: self.y1 - child.y1,
            bottom: child.y2 - self.y2,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.x1 == self.x2 || self.y1 == self.y2
    }

    pub fn to_origin(&self) -> Self {
        Self {
            x1: 0,
            y1: 0,
            x2: self.x2 - self.x1,
            y2: self.y2 - self.y1,
        }
    }

    pub fn move_(&self, dx: i32, dy: i32) -> Self {
        Self {
            x1: self.x1.saturating_add(dx),
            y1: self.y1.saturating_add(dy),
            x2: self.x2.saturating_add(dx),
            y2: self.y2.saturating_add(dy),
        }
    }

    pub fn translate(&self, x: i32, y: i32) -> (i32, i32) {
        (x.wrapping_sub(self.x1), y.wrapping_sub(self.y1))
    }

    pub fn translate_inv(&self, x: i32, y: i32) -> (i32, i32) {
        (x.wrapping_add(self.x1), y.wrapping_add(self.y1))
    }

    pub fn x1(&self) -> i32 {
        self.x1
    }

    pub fn x2(&self) -> i32 {
        self.x2
    }

    pub fn y1(&self) -> i32 {
        self.y1
    }

    pub fn y2(&self) -> i32 {
        self.y2
    }

    pub fn width(&self) -> i32 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> i32 {
        self.y2 - self.y1
    }
}
