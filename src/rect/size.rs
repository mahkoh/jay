use crate::rect::Rect;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct Size {
    width: i32,
    height: i32,
}

impl Size {
    #[expect(dead_code)]
    #[inline(always)]
    pub fn new(width: i32, height: i32) -> Option<Self> {
        if width < 0 || height < 0 {
            return None;
        }
        Some(Self { width, height })
    }

    #[inline(always)]
    pub fn new_saturating(width: i32, height: i32) -> Self {
        Self {
            width: width.max(0),
            height: height.max(0),
        }
    }

    #[expect(dead_code)]
    #[inline(always)]
    pub fn width(self) -> i32 {
        self.width
    }

    #[expect(dead_code)]
    #[inline(always)]
    pub fn height(self) -> i32 {
        self.height
    }

    #[expect(dead_code)]
    #[inline(always)]
    pub fn at_point(self, x: i32, y: i32) -> Option<Rect> {
        Rect::new_sized(x, y, self.width, self.height)
    }
}
