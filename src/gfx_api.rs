use {
    crate::{
        allocator::Allocator,
        clientmem::ClientMemOffset,
        cpu_worker::CpuWorker,
        cursor::Cursor,
        damage::DamageVisualizer,
        fixed::Fixed,
        format::Format,
        rect::{Rect, Region},
        renderer::{renderer_base::RendererBase, Renderer},
        scale::Scale,
        state::State,
        theme::Color,
        tree::{Node, OutputNode},
        utils::{clonecell::UnsafeCellCloneSafe, transform_ext::TransformExt},
        video::{dmabuf::DmaBuf, drm::sync_obj::SyncObjCtx, Modifier},
    },
    ahash::AHashMap,
    indexmap::{IndexMap, IndexSet},
    jay_config::video::{GfxApi, Transform},
    std::{
        any::Any,
        cell::Cell,
        error::Error,
        ffi::CString,
        fmt::{Debug, Formatter},
        ops::Deref,
        rc::Rc,
        sync::atomic::{AtomicU64, Ordering::Relaxed},
    },
    thiserror::Error,
    uapi::OwnedFd,
};

pub enum GfxApiOpt {
    Sync,
    FillRect(FillRect),
    CopyTexture(CopyTexture),
}

pub struct GfxRenderPass {
    pub ops: Vec<GfxApiOpt>,
    pub clear: Option<Color>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct SampleRect {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub buffer_transform: Transform,
}

impl SampleRect {
    pub fn identity() -> Self {
        Self {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
            buffer_transform: Transform::None,
        }
    }

    pub fn is_covering(&self) -> bool {
        self.x1 == 0.0 && self.y1 == 0.0 && self.x2 == 1.0 && self.y2 == 1.0
    }

    pub fn to_points(&self) -> [[f32; 2]; 4] {
        use Transform::*;
        let x1 = self.x1;
        let x2 = self.x2;
        let y1 = self.y1;
        let y2 = self.y2;
        match self.buffer_transform {
            None => [[x2, y1], [x1, y1], [x2, y2], [x1, y2]],
            Rotate90 => [
                [y1, 1.0 - x2],
                [y1, 1.0 - x1],
                [y2, 1.0 - x2],
                [y2, 1.0 - x1],
            ],
            Rotate180 => [
                [1.0 - x2, 1.0 - y1],
                [1.0 - x1, 1.0 - y1],
                [1.0 - x2, 1.0 - y2],
                [1.0 - x1, 1.0 - y2],
            ],
            Rotate270 => [
                [1.0 - y1, x2],
                [1.0 - y1, x1],
                [1.0 - y2, x2],
                [1.0 - y2, x1],
            ],
            Flip => [
                [1.0 - x2, y1],
                [1.0 - x1, y1],
                [1.0 - x2, y2],
                [1.0 - x1, y2],
            ],
            FlipRotate90 => [[y1, x2], [y1, x1], [y2, x2], [y2, x1]],
            FlipRotate180 => [
                [x2, 1.0 - y1],
                [x1, 1.0 - y1],
                [x2, 1.0 - y2],
                [x1, 1.0 - y2],
            ],
            FlipRotate270 => [
                [1.0 - y1, 1.0 - x2],
                [1.0 - y1, 1.0 - x1],
                [1.0 - y2, 1.0 - x2],
                [1.0 - y2, 1.0 - x1],
            ],
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct FramebufferRect {
    pub x1: f32,
    pub x2: f32,
    pub y1: f32,
    pub y2: f32,
    pub output_transform: Transform,
}

impl FramebufferRect {
    pub fn new(
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        transform: Transform,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            x1: 2.0 * x1 / width - 1.0,
            x2: 2.0 * x2 / width - 1.0,
            y1: 2.0 * y1 / height - 1.0,
            y2: 2.0 * y2 / height - 1.0,
            output_transform: transform,
        }
    }

    pub fn to_points(&self) -> [[f32; 2]; 4] {
        use Transform::*;
        let x1 = self.x1;
        let x2 = self.x2;
        let y1 = self.y1;
        let y2 = self.y2;
        match self.output_transform {
            None => [[x2, y1], [x1, y1], [x2, y2], [x1, y2]],
            Rotate90 => [[y1, -x2], [y1, -x1], [y2, -x2], [y2, -x1]],
            Rotate180 => [[-x2, -y1], [-x1, -y1], [-x2, -y2], [-x1, -y2]],
            Rotate270 => [[-y1, x2], [-y1, x1], [-y2, x2], [-y2, x1]],
            Flip => [[-x2, y1], [-x1, y1], [-x2, y2], [-x1, y2]],
            FlipRotate90 => [[y1, x2], [y1, x1], [y2, x2], [y2, x1]],
            FlipRotate180 => [[x2, -y1], [x1, -y1], [x2, -y2], [x1, -y2]],
            FlipRotate270 => [[-y1, -x2], [-y1, -x1], [-y2, -x2], [-y2, -x1]],
        }
    }

    pub fn is_covering(&self) -> bool {
        self.x1 == -1.0 && self.y1 == -1.0 && self.x2 == 1.0 && self.y2 == 1.0
    }
}

#[derive(Debug)]
pub struct FillRect {
    pub rect: FramebufferRect,
    pub color: Color,
}

pub struct CopyTexture {
    pub tex: Rc<dyn GfxTexture>,
    pub source: SampleRect,
    pub target: FramebufferRect,
    pub buffer_resv: Option<Rc<dyn BufferResv>>,
    pub acquire_sync: AcquireSync,
    pub release_sync: ReleaseSync,
    pub alpha: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct SyncFile(pub Rc<OwnedFd>);

impl Deref for SyncFile {
    type Target = Rc<OwnedFd>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl UnsafeCellCloneSafe for SyncFile {}

#[derive(Clone)]
pub enum AcquireSync {
    None,
    Implicit,
    SyncFile { sync_file: SyncFile },
    Unnecessary,
}

impl AcquireSync {
    pub fn from_sync_file(sync_file: Option<SyncFile>) -> Self {
        match sync_file {
            None => Self::Unnecessary,
            Some(sync_file) => Self::SyncFile { sync_file },
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ReleaseSync {
    None,
    Implicit,
    Explicit,
}

impl Debug for AcquireSync {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            AcquireSync::None => "None",
            AcquireSync::Implicit => "Implicit",
            AcquireSync::SyncFile { .. } => "SyncFile",
            AcquireSync::Unnecessary => "Unnecessary",
        };
        f.debug_struct(name).finish_non_exhaustive()
    }
}

pub trait BufferResv: Debug {
    fn set_sync_file(&self, user: BufferResvUser, sync_file: &SyncFile);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BufferResvUser(u64);

impl Default for BufferResvUser {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Relaxed))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResetStatus {
    Guilty,
    Innocent,
    Unknown,
    Other(u32),
}

pub trait GfxFramebuffer: Debug {
    fn physical_size(&self) -> (i32, i32);

    fn render(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        ops: &[GfxApiOpt],
        clear: Option<&Color>,
    ) -> Result<Option<SyncFile>, GfxError>;

    fn copy_to_shm(
        self: Rc<Self>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError>;

    fn format(&self) -> &'static Format;
}

impl dyn GfxFramebuffer {
    pub fn clear(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.clear_with(acquire_sync, release_sync, 0.0, 0.0, 0.0, 0.0)
    }

    pub fn clear_with(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.render(acquire_sync, release_sync, &[], Some(&Color { r, g, b, a }))
    }

    pub fn logical_size(&self, transform: Transform) -> (i32, i32) {
        logical_size(self.physical_size(), transform)
    }

    pub fn renderer_base<'a>(
        &self,
        ops: &'a mut Vec<GfxApiOpt>,
        scale: Scale,
        transform: Transform,
    ) -> RendererBase<'a> {
        renderer_base(self.physical_size(), ops, scale, transform)
    }

    pub fn copy_texture(
        &self,
        fb_acquire_sync: AcquireSync,
        fb_release_sync: ReleaseSync,
        texture: &Rc<dyn GfxTexture>,
        resv: Option<&Rc<dyn BufferResv>>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        x: i32,
        y: i32,
    ) -> Result<Option<SyncFile>, GfxError> {
        let mut ops = vec![];
        let scale = Scale::from_int(1);
        let mut renderer = self.renderer_base(&mut ops, scale, Transform::None);
        renderer.render_texture(
            texture,
            None,
            x,
            y,
            None,
            None,
            scale,
            None,
            resv.cloned(),
            acquire_sync,
            release_sync,
        );
        let clear = self.format().has_alpha.then_some(&Color::TRANSPARENT);
        self.render(fb_acquire_sync, fb_release_sync, &ops, clear)
    }

    pub fn render_custom(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        scale: Scale,
        clear: Option<&Color>,
        f: &mut dyn FnMut(&mut RendererBase),
    ) -> Result<Option<SyncFile>, GfxError> {
        let mut ops = vec![];
        let mut renderer = self.renderer_base(&mut ops, scale, Transform::None);
        f(&mut renderer);
        self.render(acquire_sync, release_sync, &ops, clear)
    }

    pub fn create_render_pass(
        &self,
        node: &dyn Node,
        state: &State,
        cursor_rect: Option<Rect>,
        scale: Scale,
        render_cursor: bool,
        render_hardware_cursor: bool,
        black_background: bool,
        transform: Transform,
        visualizer: Option<&DamageVisualizer>,
    ) -> GfxRenderPass {
        create_render_pass(
            self.physical_size(),
            node,
            state,
            cursor_rect,
            scale,
            render_cursor,
            render_hardware_cursor,
            black_background,
            transform,
            visualizer,
        )
    }

    pub fn perform_render_pass(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        pass: &GfxRenderPass,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.render(acquire_sync, release_sync, &pass.ops, pass.clear.as_ref())
    }

    pub fn render_output(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        node: &OutputNode,
        state: &State,
        cursor_rect: Option<Rect>,
        scale: Scale,
        render_hardware_cursor: bool,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.render_node(
            acquire_sync,
            release_sync,
            node,
            state,
            cursor_rect,
            scale,
            true,
            render_hardware_cursor,
            node.has_fullscreen(),
            node.global.persistent.transform.get(),
        )
    }

    pub fn render_node(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        node: &dyn Node,
        state: &State,
        cursor_rect: Option<Rect>,
        scale: Scale,
        render_cursor: bool,
        render_hardware_cursor: bool,
        black_background: bool,
        transform: Transform,
    ) -> Result<Option<SyncFile>, GfxError> {
        let pass = self.create_render_pass(
            node,
            state,
            cursor_rect,
            scale,
            render_cursor,
            render_hardware_cursor,
            black_background,
            transform,
            None,
        );
        self.perform_render_pass(acquire_sync, release_sync, &pass)
    }

    pub fn render_hardware_cursor(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cursor: &dyn Cursor,
        state: &State,
        scale: Scale,
        transform: Transform,
    ) -> Result<Option<SyncFile>, GfxError> {
        let mut ops = vec![];
        let mut renderer = Renderer {
            base: self.renderer_base(&mut ops, scale, transform),
            state,
            logical_extents: Rect::new_empty(0, 0),
            pixel_extents: {
                let (width, height) = self.logical_size(transform);
                Rect::new(0, 0, width, height).unwrap()
            },
        };
        cursor.render_hardware_cursor(&mut renderer);
        self.render(acquire_sync, release_sync, &ops, Some(&Color::TRANSPARENT))
    }
}

pub trait GfxImage {
    fn to_framebuffer(self: Rc<Self>) -> Result<Rc<dyn GfxFramebuffer>, GfxError>;

    fn to_texture(self: Rc<Self>) -> Result<Rc<dyn GfxTexture>, GfxError>;

    fn width(&self) -> i32;
    fn height(&self) -> i32;
}

pub trait GfxTexture: Debug {
    fn size(&self) -> (i32, i32);
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Rc<Self>) -> Rc<dyn Any>;
    fn read_pixels(
        self: Rc<Self>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError>;
    fn dmabuf(&self) -> Option<&DmaBuf>;
    fn format(&self) -> &'static Format;
}

pub trait ShmGfxTexture: GfxTexture {
    fn into_texture(self: Rc<Self>) -> Rc<dyn GfxTexture>;
}

pub trait AsyncShmGfxTextureCallback {
    fn completed(self: Rc<Self>, res: Result<(), GfxError>);
}

pub trait AsyncShmGfxTextureUploadCancellable {
    fn cancel(&self, id: u64);
}

pub struct PendingShmUpload {
    cancel: Rc<dyn AsyncShmGfxTextureUploadCancellable>,
    id: u64,
}

pub trait AsyncShmGfxTexture: GfxTexture {
    fn async_upload(
        self: Rc<Self>,
        callback: Rc<dyn AsyncShmGfxTextureCallback>,
        mem: &Rc<ClientMemOffset>,
        damage: Region,
    ) -> Result<Option<PendingShmUpload>, GfxError>;

    fn sync_upload(self: Rc<Self>, shm: &[Cell<u8>], damage: Region) -> Result<(), GfxError>;

    fn compatible_with(
        &self,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> bool;

    fn into_texture(self: Rc<Self>) -> Rc<dyn GfxTexture>;
}

pub trait GfxContext: Debug {
    fn reset_status(&self) -> Option<ResetStatus>;

    fn render_node(&self) -> Option<Rc<CString>>;

    fn formats(&self) -> Rc<AHashMap<u32, GfxFormat>>;

    fn dmabuf_fb(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        self.dmabuf_img(buf)?.to_framebuffer()
    }

    fn dmabuf_img(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxImage>, GfxError>;

    fn shmem_texture(
        self: Rc<Self>,
        old: Option<Rc<dyn ShmGfxTexture>>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        damage: Option<&[Rect]>,
    ) -> Result<Rc<dyn ShmGfxTexture>, GfxError>;

    fn async_shmem_texture(
        self: Rc<Self>,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        cpu_worker: &Rc<CpuWorker>,
    ) -> Result<Rc<dyn AsyncShmGfxTexture>, GfxError>;

    fn allocator(&self) -> Rc<dyn Allocator>;

    fn gfx_api(&self) -> GfxApi;

    fn create_fb(
        self: Rc<Self>,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
    ) -> Result<Rc<dyn GfxFramebuffer>, GfxError>;

    fn sync_obj_ctx(&self) -> Option<&Rc<SyncObjCtx>>;
}

#[derive(Clone, Debug)]
pub struct GfxWriteModifier {
    pub needs_render_usage: bool,
}

pub fn needs_render_usage<'a>(mut modifiers: impl Iterator<Item = &'a GfxWriteModifier>) -> bool {
    modifiers.any(|m| m.needs_render_usage)
}

#[derive(Debug)]
pub struct GfxFormat {
    pub format: &'static Format,
    pub read_modifiers: IndexSet<Modifier>,
    pub write_modifiers: IndexMap<Modifier, GfxWriteModifier>,
}

#[derive(Error)]
#[error(transparent)]
pub struct GfxError(pub Box<dyn Error + Send>);

impl Debug for GfxError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl GfxFormat {
    pub fn cross_intersect(&self, other: &GfxFormat) -> GfxFormat {
        assert_eq!(self.format, other.format);
        GfxFormat {
            format: self.format,
            read_modifiers: self
                .read_modifiers
                .iter()
                .copied()
                .filter(|m| other.write_modifiers.contains_key(m))
                .collect(),
            write_modifiers: self
                .write_modifiers
                .iter()
                .map(|(m, v)| (*m, v.clone()))
                .filter(|(m, _)| other.read_modifiers.contains(m))
                .collect(),
        }
    }
}

pub fn cross_intersect_formats(
    local: &AHashMap<u32, GfxFormat>,
    remote: &AHashMap<u32, GfxFormat>,
) -> AHashMap<u32, GfxFormat> {
    let mut res = AHashMap::new();
    for lf in local.values() {
        if let Some(rf) = remote.get(&lf.format.drm) {
            let f = lf.cross_intersect(rf);
            if f.read_modifiers.is_empty() && f.write_modifiers.is_empty() {
                continue;
            }
            res.insert(f.format.drm, f);
        }
    }
    res
}

impl PendingShmUpload {
    pub fn new(cancel: Rc<dyn AsyncShmGfxTextureUploadCancellable>, id: u64) -> Self {
        Self { cancel, id }
    }
}

impl Drop for PendingShmUpload {
    fn drop(&mut self) {
        self.cancel.cancel(self.id);
    }
}

pub fn create_render_pass(
    physical_size: (i32, i32),
    node: &dyn Node,
    state: &State,
    cursor_rect: Option<Rect>,
    scale: Scale,
    render_cursor: bool,
    render_hardware_cursor: bool,
    black_background: bool,
    transform: Transform,
    visualizer: Option<&DamageVisualizer>,
) -> GfxRenderPass {
    let mut ops = vec![];
    let mut renderer = Renderer {
        base: renderer_base(physical_size, &mut ops, scale, transform),
        state,
        logical_extents: node.node_absolute_position().at_point(0, 0),
        pixel_extents: {
            let (width, height) = logical_size(physical_size, transform);
            Rect::new(0, 0, width, height).unwrap()
        },
    };
    node.node_render(&mut renderer, 0, 0, None);
    if let Some(rect) = cursor_rect {
        let seats = state.globals.lock_seats();
        for seat in seats.values() {
            let (x, y) = seat.pointer_cursor().position_int();
            if let Some(im) = seat.input_method() {
                for (_, popup) in &im.popups {
                    if popup.surface.node_visible() {
                        let pos = popup.surface.buffer_abs_pos.get();
                        let extents = popup.surface.extents.get().move_(pos.x1(), pos.y1());
                        if extents.intersects(&rect) {
                            let (x, y) = rect.translate(pos.x1(), pos.y1());
                            renderer.render_surface(&popup.surface, x, y, None);
                        }
                    }
                }
            }
            if let Some(drag) = seat.toplevel_drag() {
                drag.render(&mut renderer, &rect, x, y);
            }
            if let Some(dnd_icon) = seat.dnd_icon() {
                dnd_icon.render(&mut renderer, &rect, x, y);
            }
            if render_cursor {
                let cursor_user_group = seat.cursor_group();
                if render_hardware_cursor || !cursor_user_group.hardware_cursor() {
                    if let Some(cursor_user) = cursor_user_group.active() {
                        if let Some(cursor) = cursor_user.get() {
                            cursor.tick();
                            let (mut x, mut y) = cursor_user.position();
                            x -= Fixed::from_int(rect.x1());
                            y -= Fixed::from_int(rect.y1());
                            cursor.render(&mut renderer, x, y);
                        }
                    }
                }
            }
        }
    }
    if let Some(visualizer) = visualizer {
        if let Some(cursor_rect) = cursor_rect {
            visualizer.render(&cursor_rect, &mut renderer.base);
        }
    }
    let c = match black_background {
        true => Color::SOLID_BLACK,
        false => state.theme.colors.background.get(),
    };
    GfxRenderPass {
        ops,
        clear: Some(c),
    }
}

pub fn renderer_base<'a>(
    physical_size: (i32, i32),
    ops: &'a mut Vec<GfxApiOpt>,
    scale: Scale,
    transform: Transform,
) -> RendererBase<'a> {
    let (width, height) = logical_size(physical_size, transform);
    RendererBase {
        ops,
        scaled: scale != 1,
        scale,
        scalef: scale.to_f64(),
        transform,
        fb_width: width as _,
        fb_height: height as _,
    }
}

pub fn logical_size(physical_size: (i32, i32), transform: Transform) -> (i32, i32) {
    transform.maybe_swap(physical_size)
}
