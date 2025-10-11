mod region;
mod size;

#[cfg(test)]
mod tests;

pub use {
    crate::rect::size::Size,
    region::{DamageQueue, RegionBuilder},
};
use {
    jay_algorithms::rect::{NoTag, RectRaw, Tag},
    smallvec::SmallVec,
    std::fmt::{Debug, Formatter},
};

#[derive(Copy, Clone, Eq, PartialEq, Default)]
#[repr(transparent)]
pub struct Rect<T = NoTag>
where
    T: Tag,
{
    raw: RectRaw<T>,
}

#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct Region<T = NoTag>
where
    T: Tag,
{
    rects: SmallVec<[RectRaw<T>; 1]>,
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

impl<T> Rect<T>
where
    T: Tag,
{
    pub fn untag(&self) -> Rect {
        Rect {
            raw: RectRaw {
                x1: self.raw.x1,
                y1: self.raw.y1,
                x2: self.raw.x2,
                y2: self.raw.y2,
                tag: NoTag,
            },
        }
    }
}

impl Rect {
    pub fn new_empty(x: i32, y: i32) -> Self {
        Self {
            raw: RectRaw {
                x1: x,
                y1: y,
                x2: x,
                y2: y,
                tag: NoTag,
            },
        }
    }

    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Option<Self> {
        if x2 < x1 || y2 < y1 {
            return None;
        }
        Some(Self {
            raw: RectRaw {
                x1,
                y1,
                x2,
                y2,
                tag: NoTag,
            },
        })
    }

    #[track_caller]
    pub fn new_unchecked(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self::new(x1, y1, x2, y2).unwrap()
    }

    #[cfg_attr(not(test), expect(dead_code))]
    fn new_unchecked_danger(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            raw: RectRaw {
                x1,
                y1,
                x2,
                y2,
                tag: NoTag,
            },
        }
    }

    pub fn new_sized(x1: i32, y1: i32, width: i32, height: i32) -> Option<Self> {
        if width < 0 || height < 0 {
            return None;
        }
        Self::new(x1, y1, x1 + width, y1 + height)
    }

    #[track_caller]
    pub fn new_sized_unchecked(x1: i32, y1: i32, width: i32, height: i32) -> Self {
        Self::new_sized(x1, y1, width, height).unwrap()
    }

    pub fn union(&self, other: Self) -> Self {
        Self {
            raw: RectRaw {
                x1: self.raw.x1.min(other.raw.x1),
                y1: self.raw.y1.min(other.raw.y1),
                x2: self.raw.x2.max(other.raw.x2),
                y2: self.raw.y2.max(other.raw.y2),
                tag: NoTag,
            },
        }
    }

    pub fn intersect(&self, other: Self) -> Self {
        let x1 = self.raw.x1.max(other.raw.x1);
        let y1 = self.raw.y1.max(other.raw.y1);
        let x2 = self.raw.x2.min(other.raw.x2).max(x1);
        let y2 = self.raw.y2.min(other.raw.y2).max(y1);
        Self {
            raw: RectRaw {
                x1,
                y1,
                x2,
                y2,
                tag: NoTag,
            },
        }
    }

    pub fn with_size(&self, width: i32, height: i32) -> Option<Self> {
        Self::new_sized(self.raw.x1, self.raw.y1, width, height)
    }

    pub fn with_tag(&self, tag: u32) -> Rect<u32> {
        Rect {
            raw: RectRaw {
                x1: self.raw.x1,
                y1: self.raw.y1,
                x2: self.raw.x2,
                y2: self.raw.y2,
                tag,
            },
        }
    }
}

impl<T> Rect<T>
where
    T: Tag,
{
    #[cfg_attr(not(test), expect(dead_code))]
    fn new_unchecked_danger_tagged(x1: i32, y1: i32, x2: i32, y2: i32, tag: T) -> Self {
        Self {
            raw: RectRaw {
                x1,
                y1,
                x2,
                y2,
                tag,
            },
        }
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.raw.x1 < other.raw.x2
            && other.raw.x1 < self.raw.x2
            && self.raw.y1 < other.raw.y2
            && other.raw.y1 < self.raw.y2
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.raw.x1 <= x && self.raw.y1 <= y && self.raw.x2 > x && self.raw.y2 > y
    }

    pub fn dist_squared(&self, x: i32, y: i32) -> i32 {
        let mut dx = 0;
        if self.raw.x1 > x {
            dx = self.raw.x1 - x;
        } else if self.raw.x2 < x {
            dx = x - self.raw.x2;
        }
        let mut dy = 0;
        if self.raw.y1 > y {
            dy = self.raw.y1 - y;
        } else if self.raw.y2 < y {
            dy = y - self.raw.y2;
        }
        dx * dx + dy * dy
    }

    #[expect(dead_code)]
    pub fn contains_rect<U>(&self, rect: &Rect<U>) -> bool
    where
        U: Tag,
    {
        self.raw.x1 <= rect.raw.x1
            && self.raw.y1 <= rect.raw.x1
            && rect.raw.x2 <= self.raw.x2
            && rect.raw.y2 <= self.raw.y2
    }

    pub fn get_overflow<U>(&self, child: &Rect<U>) -> RectOverflow
    where
        U: Tag,
    {
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

    #[expect(dead_code)]
    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    #[expect(dead_code)]
    pub fn to_origin(&self) -> Self {
        Self {
            raw: RectRaw {
                x1: 0,
                y1: 0,
                x2: self.raw.x2 - self.raw.x1,
                y2: self.raw.y2 - self.raw.y1,
                tag: self.raw.tag,
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
                tag: self.raw.tag,
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
                tag: self.raw.tag,
            },
        }
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

    pub fn size2(&self) -> Size {
        Size::new_unchecked(self.width(), self.height())
    }

    pub fn center(&self) -> (i32, i32) {
        (
            self.raw.x1 + self.width() / 2,
            self.raw.y1 + self.height() / 2,
        )
    }

    pub fn tag(&self) -> T {
        self.raw.tag
    }
}
