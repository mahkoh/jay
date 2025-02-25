pub mod region;

use {
    smallvec::SmallVec,
    std::fmt::{Debug, Formatter},
};

pub trait Tag: Copy + Eq + Ord + Debug + Default + Sized {
    const IS_SIGNIFICANT: bool;

    fn constrain(self) -> Self;
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Default)]
pub struct NoTag;

impl Tag for NoTag {
    const IS_SIGNIFICANT: bool = false;

    #[inline(always)]
    fn constrain(self) -> Self {
        Self
    }
}

impl Tag for u32 {
    const IS_SIGNIFICANT: bool = true;

    #[inline(always)]
    fn constrain(self) -> Self {
        self & 1
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub struct RectRaw<T = NoTag>
where
    T: Tag,
{
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub tag: T,
}

impl<T> Debug for RectRaw<T>
where
    T: Tag,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Rect");
        debug
            .field("x1", &self.x1)
            .field("y1", &self.y1)
            .field("x2", &self.x2)
            .field("y2", &self.y2)
            .field("width", &(self.x2 - self.x1))
            .field("height", &(self.y2 - self.y1));
        if T::IS_SIGNIFICANT {
            debug.field("tag", &self.tag);
        }
        debug.finish()
    }
}

impl<T> RectRaw<T>
where
    T: Tag,
{
    fn is_empty(&self) -> bool {
        self.x1 == self.x2 || self.y1 == self.y2
    }
}

type Container<T = NoTag> = SmallVec<[RectRaw<T>; 1]>;
