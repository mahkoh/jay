use {
    crate::{format::Format, render::Texture, theme::Color},
    std::rc::Rc,
};

pub enum GfxApiOpt {
    Sync,
    Clear(Clear),
    FillRect(FillRect),
    CopyTexture(CopyTexture),
}

#[derive(Default, Debug, Copy, Clone)]
pub struct BufferPoint {
    pub x: f32,
    pub y: f32,
}

impl BufferPoint {
    pub fn is_leq_1(&self) -> bool {
        self.x <= 1.0 && self.y <= 1.0
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct BufferPoints {
    pub top_left: BufferPoint,
    pub top_right: BufferPoint,
    pub bottom_left: BufferPoint,
    pub bottom_right: BufferPoint,
}

impl BufferPoints {
    pub fn norm(&self, width: f32, height: f32) -> Self {
        Self {
            top_left: BufferPoint {
                x: self.top_left.x / width,
                y: self.top_left.y / height,
            },
            top_right: BufferPoint {
                x: self.top_right.x / width,
                y: self.top_right.y / height,
            },
            bottom_left: BufferPoint {
                x: self.bottom_left.x / width,
                y: self.bottom_left.y / height,
            },
            bottom_right: BufferPoint {
                x: self.bottom_right.x / width,
                y: self.bottom_right.y / height,
            },
        }
    }

    pub fn is_leq_1(&self) -> bool {
        self.top_left.is_leq_1()
            && self.top_right.is_leq_1()
            && self.bottom_left.is_leq_1()
            && self.bottom_right.is_leq_1()
    }
}

pub struct AbsoluteRect {
    pub x1: f32,
    pub x2: f32,
    pub y1: f32,
    pub y2: f32,
}

pub struct Clear {
    pub color: Color,
}

pub struct FillRect {
    pub rect: AbsoluteRect,
    pub color: Color,
}

pub struct CopyTexture {
    pub tex: Rc<Texture>,
    pub format: &'static Format,
    pub source: BufferPoints,
    pub target: AbsoluteRect,
}
