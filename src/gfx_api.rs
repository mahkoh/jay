use {
    crate::{
        cursor::Cursor,
        format::Format,
        rect::Rect,
        renderer::{renderer_base::RendererBase, RenderResult},
        scale::Scale,
        state::State,
        theme::Color,
        tree::Node,
        video::{dmabuf::DmaBuf, gbm::GbmDevice},
    },
    ahash::AHashMap,
    std::{
        any::Any,
        cell::Cell,
        error::Error,
        ffi::CString,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
    thiserror::Error,
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
    pub tex: Rc<dyn GfxTexture>,
    pub format: &'static Format,
    pub source: BufferPoints,
    pub target: AbsoluteRect,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResetStatus {
    Guilty,
    Innocent,
    Unknown,
    Other(u32),
}

pub trait GfxFramebuffer: Debug {
    fn as_any(&self) -> &dyn Any;

    fn take_render_ops(&self) -> Vec<GfxApiOpt>;

    fn clear(&self);

    fn clear_with(&self, r: f32, g: f32, b: f32, a: f32);

    fn copy_texture(
        &self,
        state: &State,
        texture: &Rc<dyn GfxTexture>,
        x: i32,
        y: i32,
        alpha: bool,
    );

    fn copy_to_shm(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: &Format,
        shm: &[Cell<u8>],
    );

    fn render_custom(&self, scale: Scale, f: &mut dyn FnMut(&mut RendererBase));

    fn render(
        &self,
        node: &dyn Node,
        state: &State,
        cursor_rect: Option<Rect>,
        result: Option<&mut RenderResult>,
        scale: Scale,
        render_hardware_cursor: bool,
    );

    fn render_hardware_cursor(&self, cursor: &dyn Cursor, state: &State, scale: Scale);
}

pub trait GfxImage {
    fn to_framebuffer(self: Rc<Self>) -> Result<Rc<dyn GfxFramebuffer>, GfxError>;

    fn to_texture(self: Rc<Self>) -> Result<Rc<dyn GfxTexture>, GfxError>;

    fn width(&self) -> i32;
    fn height(&self) -> i32;
}

pub trait GfxTexture: Debug {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn as_any(&self) -> &dyn Any;
}

pub trait GfxContext: Debug {
    fn reset_status(&self) -> Option<ResetStatus>;

    fn supports_external_texture(&self) -> bool;

    fn render_node(&self) -> Rc<CString>;

    fn formats(&self) -> Rc<AHashMap<u32, GfxFormat>>;

    fn dmabuf_fb(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxFramebuffer>, GfxError>;

    fn dmabuf_img(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxImage>, GfxError>;

    fn shmem_texture(
        self: Rc<Self>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> Result<Rc<dyn GfxTexture>, GfxError>;

    fn gbm(&self) -> &GbmDevice;
}

#[derive(Debug)]
pub struct GfxFormat {
    pub format: &'static Format,
    pub implicit_external_only: bool,
    pub modifiers: AHashMap<u64, GfxModifier>,
}

#[derive(Debug)]
pub struct GfxModifier {
    pub modifier: u64,
    pub external_only: bool,
}

#[derive(Error)]
#[error(transparent)]
pub struct GfxError(pub Box<dyn Error>);

impl Debug for GfxError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
