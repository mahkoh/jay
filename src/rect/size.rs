use crate::rect::Rect;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct Size {
    width: i32,
    height: i32,
}

impl Size {
    #[inline(always)]
    pub fn new(width: i32, height: i32) -> Option<Self> {
        if width < 0 || height < 0 {
            return None;
        }
        Some(Self { width, height })
    }

    #[inline(always)]
    pub fn new_unchecked(width: i32, height: i32) -> Self {
        Self { width, height }
    }

    #[inline(always)]
    pub fn width(self) -> i32 {
        self.width
    }

    #[inline(always)]
    pub fn height(self) -> i32 {
        self.height
    }

    #[inline(always)]
    #[expect(dead_code)]
    pub fn at_point(self, x: i32, y: i32) -> Option<Rect> {
        Rect::new_sized(x, y, self.width, self.height)
    }
}
