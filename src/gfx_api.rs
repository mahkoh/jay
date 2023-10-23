use {
    crate::{
        cursor::Cursor,
        fixed::Fixed,
        format::{Format, ARGB8888, XRGB8888},
        rect::Rect,
        renderer::{renderer_base::RendererBase, RenderResult, Renderer},
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

    fn size(&self) -> (i32, i32);

    fn render(&self, ops: Vec<GfxApiOpt>, clear: Option<&Color>);

    fn copy_to_shm(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: &Format,
        shm: &[Cell<u8>],
    );
}

impl dyn GfxFramebuffer {
    pub fn clear(&self) {
        self.clear_with(0.0, 0.0, 0.0, 0.0);
    }

    pub fn clear_with(&self, r: f32, g: f32, b: f32, a: f32) {
        let ops = self.take_render_ops();
        self.render(ops, Some(&Color { r, g, b, a }));
    }

    pub fn copy_texture(
        &self,
        state: &State,
        texture: &Rc<dyn GfxTexture>,
        x: i32,
        y: i32,
        alpha: bool,
    ) {
        let mut ops = self.take_render_ops();
        let scale = Scale::from_int(1);
        let (width, height) = self.size();
        let extents = Rect::new_sized(0, 0, width, height).unwrap();
        let mut renderer = Renderer {
            base: RendererBase {
                ops: &mut ops,
                scaled: false,
                scale,
                scalef: 1.0,
            },
            state,
            result: None,
            logical_extents: extents,
            physical_extents: extents,
        };
        let (format, clear) = match alpha {
            true => (ARGB8888, Some(&Color::TRANSPARENT)),
            false => (XRGB8888, None),
        };
        renderer
            .base
            .render_texture(texture, x, y, format, None, None, scale, i32::MAX, i32::MAX);
        self.render(ops, clear);
    }

    pub fn render_custom(&self, scale: Scale, f: &mut dyn FnMut(&mut RendererBase)) {
        let mut ops = self.take_render_ops();
        let mut renderer = RendererBase {
            ops: &mut ops,
            scaled: scale != 1,
            scale,
            scalef: scale.to_f64(),
        };
        f(&mut renderer);
        self.render(ops, None);
    }

    pub fn render_node(
        &self,
        node: &dyn Node,
        state: &State,
        cursor_rect: Option<Rect>,
        result: Option<&mut RenderResult>,
        scale: Scale,
        render_hardware_cursor: bool,
    ) {
        let mut ops = self.take_render_ops();
        let (width, height) = self.size();
        let mut renderer = Renderer {
            base: RendererBase {
                ops: &mut ops,
                scaled: scale != 1,
                scale,
                scalef: scale.to_f64(),
            },
            state,
            result,
            logical_extents: node.node_absolute_position().at_point(0, 0),
            physical_extents: Rect::new(0, 0, width, height).unwrap(),
        };
        node.node_render(&mut renderer, 0, 0, i32::MAX, i32::MAX);
        if let Some(rect) = cursor_rect {
            let seats = state.globals.lock_seats();
            for seat in seats.values() {
                if let Some(cursor) = seat.get_cursor() {
                    let (mut x, mut y) = seat.get_position();
                    if let Some(dnd_icon) = seat.dnd_icon() {
                        let extents = dnd_icon.extents.get().move_(
                            x.round_down() + dnd_icon.buf_x.get(),
                            y.round_down() + dnd_icon.buf_y.get(),
                        );
                        if extents.intersects(&rect) {
                            let (x, y) = rect.translate(extents.x1(), extents.y1());
                            renderer.render_surface(&dnd_icon, x, y, i32::MAX, i32::MAX);
                        }
                    }
                    if render_hardware_cursor || !seat.hardware_cursor() {
                        cursor.tick();
                        x -= Fixed::from_int(rect.x1());
                        y -= Fixed::from_int(rect.y1());
                        cursor.render(&mut renderer, x, y);
                    }
                }
            }
        }
        let c = state.theme.colors.background.get();
        self.render(ops, Some(&c));
    }

    pub fn render_hardware_cursor(&self, cursor: &dyn Cursor, state: &State, scale: Scale) {
        let mut ops = self.take_render_ops();
        let (width, height) = self.size();
        let mut renderer = Renderer {
            base: RendererBase {
                ops: &mut ops,
                scaled: scale != 1,
                scale,
                scalef: scale.to_f64(),
            },
            state,
            result: None,
            logical_extents: Rect::new_empty(0, 0),
            physical_extents: Rect::new(0, 0, width, height).unwrap(),
        };
        cursor.render_hardware_cursor(&mut renderer);
        self.render(ops, Some(&Color::TRANSPARENT));
    }
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
