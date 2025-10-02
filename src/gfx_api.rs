use {
    crate::{
        allocator::Allocator,
        cmm::cmm_description::{ColorDescription, LinearColorDescription},
        cpu_worker::CpuWorker,
        cursor::Cursor,
        damage::DamageVisualizer,
        fixed::Fixed,
        format::Format,
        rect::{Rect, Region},
        renderer::{Renderer, renderer_base::RendererBase},
        scale::Scale,
        state::State,
        theme::Color,
        tree::{Node, OutputNode},
        utils::{clonecell::UnsafeCellCloneSafe, transform_ext::TransformExt},
        video::{Modifier, dmabuf::DmaBuf, drm::sync_obj::SyncObjCtx},
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
    pub clear_cd: Rc<LinearColorDescription>,
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

#[derive(Copy, Clone, Debug, PartialEq)]
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

    pub fn to_rect(&self, width: f32, height: f32) -> Rect {
        let mut x1 = self.x1;
        let mut x2 = self.x2;
        let mut y1 = self.y1;
        let mut y2 = self.y2;
        (x1, y1, x2, y2) = match self.output_transform {
            Transform::None => (x1, y1, x2, y2),
            Transform::Rotate90 => (y1, -x2, y2, -x1),
            Transform::Rotate180 => (-x2, -y2, -x1, -y1),
            Transform::Rotate270 => (-y2, x1, -y1, x2),
            Transform::Flip => (-x2, y1, -x1, y2),
            Transform::FlipRotate90 => (y1, x1, y2, x2),
            Transform::FlipRotate180 => (x1, -y2, x2, -y1),
            Transform::FlipRotate270 => (-y2, -x2, -y1, -x1),
        };
        let x1 = ((x1 + 1.0) / 2.0 * width).round() as i32;
        let x2 = ((x2 + 1.0) / 2.0 * width).round() as i32;
        let y1 = ((y1 + 1.0) / 2.0 * height).round() as i32;
        let y2 = ((y2 + 1.0) / 2.0 * height).round() as i32;
        Rect::new(x1, y1, x2, y2).unwrap_or_default()
    }
}

#[derive(Debug)]
pub struct FillRect {
    pub rect: FramebufferRect,
    pub color: Color,
    pub alpha: Option<f32>,
    pub cd: Rc<LinearColorDescription>,
}

impl FillRect {
    pub fn effective_color(&self) -> Color {
        let mut color = self.color;
        if let Some(alpha) = self.alpha {
            color = color * alpha;
        }
        color
    }
}

pub struct CopyTexture {
    pub tex: Rc<dyn GfxTexture>,
    pub source: SampleRect,
    pub target: FramebufferRect,
    pub buffer_resv: Option<Rc<dyn BufferResv>>,
    pub acquire_sync: AcquireSync,
    pub release_sync: ReleaseSync,
    pub alpha: Option<f32>,
    pub opaque: bool,
    pub cd: Rc<ColorDescription>,
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

pub trait GfxBlendBuffer: Any + Debug {}

pub trait GfxFramebuffer: Debug {
    fn physical_size(&self) -> (i32, i32);

    fn render_with_region(
        self: Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cd: &Rc<ColorDescription>,
        ops: &[GfxApiOpt],
        clear: Option<&Color>,
        clear_cd: &Rc<LinearColorDescription>,
        region: &Region,
        blend_buffer: Option<&Rc<dyn GfxBlendBuffer>>,
        blend_cd: &Rc<ColorDescription>,
    ) -> Result<Option<SyncFile>, GfxError>;

    fn format(&self) -> &'static Format;

    fn full_region(&self) -> Region {
        let (width, height) = self.physical_size();
        Region::new(Rect::new_sized_unchecked(0, 0, width, height))
    }
}

pub trait GfxInternalFramebuffer: GfxFramebuffer {
    fn stride(&self) -> i32;

    fn staging_size(&self) -> usize;

    fn download(
        self: Rc<Self>,
        staging: &Rc<dyn GfxStagingBuffer>,
        callback: Rc<dyn AsyncShmGfxTextureCallback>,
        mem: Rc<dyn ShmMemory>,
        damage: Region,
    ) -> Result<Option<PendingShmTransfer>, GfxError>;
}

impl dyn GfxFramebuffer {
    pub fn render(
        self: &Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cd: &Rc<ColorDescription>,
        ops: &[GfxApiOpt],
        clear: Option<&Color>,
        clear_cd: &Rc<LinearColorDescription>,
        blend_buffer: Option<&Rc<dyn GfxBlendBuffer>>,
        blend_cd: &Rc<ColorDescription>,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.clone().render_with_region(
            acquire_sync,
            release_sync,
            cd,
            ops,
            clear,
            clear_cd,
            &self.full_region(),
            blend_buffer,
            blend_cd,
        )
    }

    pub fn clear(
        self: &Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cd: &Rc<ColorDescription>,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.clear_with(
            acquire_sync,
            release_sync,
            cd,
            &Color::TRANSPARENT,
            &cd.linear,
        )
    }

    pub fn clear_with(
        self: &Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cd: &Rc<ColorDescription>,
        color: &Color,
        color_cd: &Rc<LinearColorDescription>,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.render(
            acquire_sync,
            release_sync,
            cd,
            &[],
            Some(color),
            color_cd,
            None,
            cd,
        )
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
        self: &Rc<Self>,
        fb_acquire_sync: AcquireSync,
        fb_release_sync: ReleaseSync,
        fb_cd: &Rc<ColorDescription>,
        texture: &Rc<dyn GfxTexture>,
        texture_cd: &Rc<ColorDescription>,
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
            false,
            texture_cd,
        );
        let clear = self.format().has_alpha.then_some(&Color::TRANSPARENT);
        self.render(
            fb_acquire_sync,
            fb_release_sync,
            fb_cd,
            &ops,
            clear,
            &fb_cd.linear,
            None,
            fb_cd,
        )
    }

    pub fn render_custom(
        self: &Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cd: &Rc<ColorDescription>,
        scale: Scale,
        clear: Option<&Color>,
        clear_cd: &Rc<LinearColorDescription>,
        blend_buffer: Option<&Rc<dyn GfxBlendBuffer>>,
        blend_cd: &Rc<ColorDescription>,
        f: &mut dyn FnMut(&mut RendererBase),
    ) -> Result<Option<SyncFile>, GfxError> {
        let mut ops = vec![];
        let mut renderer = self.renderer_base(&mut ops, scale, Transform::None);
        f(&mut renderer);
        self.render(
            acquire_sync,
            release_sync,
            cd,
            &ops,
            clear,
            clear_cd,
            blend_buffer,
            blend_cd,
        )
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
        fill_black_in_grace_period: bool,
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
            fill_black_in_grace_period,
            transform,
            visualizer,
        )
    }

    pub fn perform_render_pass(
        self: &Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cd: &Rc<ColorDescription>,
        pass: &GfxRenderPass,
        region: &Region,
        blend_buffer: Option<&Rc<dyn GfxBlendBuffer>>,
        blend_cd: &Rc<ColorDescription>,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.clone().render_with_region(
            acquire_sync,
            release_sync,
            cd,
            &pass.ops,
            pass.clear.as_ref(),
            &pass.clear_cd,
            region,
            blend_buffer,
            blend_cd,
        )
    }

    pub fn render_output(
        self: &Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cd: &Rc<ColorDescription>,
        node: &OutputNode,
        state: &State,
        cursor_rect: Option<Rect>,
        scale: Scale,
        render_hardware_cursor: bool,
        fill_black_in_grace_period: bool,
        blend_buffer: Option<&Rc<dyn GfxBlendBuffer>>,
        blend_cd: &Rc<ColorDescription>,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.render_node(
            acquire_sync,
            release_sync,
            cd,
            node,
            state,
            cursor_rect,
            scale,
            true,
            render_hardware_cursor,
            node.has_fullscreen(),
            fill_black_in_grace_period,
            node.global.persistent.transform.get(),
            blend_buffer,
            blend_cd,
        )
    }

    pub fn render_node(
        self: &Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cd: &Rc<ColorDescription>,
        node: &dyn Node,
        state: &State,
        cursor_rect: Option<Rect>,
        scale: Scale,
        render_cursor: bool,
        render_hardware_cursor: bool,
        black_background: bool,
        fill_black_in_grace_period: bool,
        transform: Transform,
        blend_buffer: Option<&Rc<dyn GfxBlendBuffer>>,
        blend_cd: &Rc<ColorDescription>,
    ) -> Result<Option<SyncFile>, GfxError> {
        let pass = self.create_render_pass(
            node,
            state,
            cursor_rect,
            scale,
            render_cursor,
            render_hardware_cursor,
            black_background,
            fill_black_in_grace_period,
            transform,
            None,
        );
        self.perform_render_pass(
            acquire_sync,
            release_sync,
            cd,
            &pass,
            &self.full_region(),
            blend_buffer,
            blend_cd,
        )
    }

    pub fn render_hardware_cursor(
        self: &Rc<Self>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        cursor: &dyn Cursor,
        state: &State,
        scale: Scale,
        transform: Transform,
        cd: &Rc<ColorDescription>,
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
            icons: None,
        };
        cursor.render_hardware_cursor(&mut renderer);
        self.render(
            acquire_sync,
            release_sync,
            cd,
            &ops,
            Some(&Color::TRANSPARENT),
            &cd.linear,
            None,
            cd,
        )
    }
}

pub trait GfxImage {
    fn to_framebuffer(self: Rc<Self>) -> Result<Rc<dyn GfxFramebuffer>, GfxError>;

    fn to_texture(self: Rc<Self>) -> Result<Rc<dyn GfxTexture>, GfxError>;

    fn width(&self) -> i32;
    fn height(&self) -> i32;
}

pub trait GfxTexture: Any + Debug {
    fn size(&self) -> (i32, i32);
    fn dmabuf(&self) -> Option<&DmaBuf>;
    fn format(&self) -> &'static Format;
}

pub trait ShmGfxTexture: GfxTexture {}

pub trait AsyncShmGfxTextureCallback {
    fn completed(self: Rc<Self>, res: Result<(), GfxError>);
}

bitflags! {
    StagingBufferUsecase: u32;
        STAGING_UPLOAD   = 1 << 0,
        STAGING_DOWNLOAD = 1 << 1,
}

pub trait GfxStagingBuffer: Any {
    fn size(&self) -> usize;
}

pub trait GfxBuffer: Any {}

pub trait AsyncShmGfxTextureTransferCancellable {
    fn cancel(&self, id: u64);
}

pub struct PendingShmTransfer {
    cancel: Rc<dyn AsyncShmGfxTextureTransferCancellable>,
    id: u64,
}

pub trait ShmMemory {
    fn len(&self) -> usize;
    fn safe_access(&self) -> ShmMemoryBacking;
    fn access(&self, f: &mut dyn FnMut(&[Cell<u8>])) -> Result<(), Box<dyn Error + Sync + Send>>;
}

pub enum ShmMemoryBacking {
    Ptr(*const [Cell<u8>]),
    Fd(Rc<OwnedFd>, usize),
}

impl ShmMemory for Vec<Cell<u8>> {
    fn len(&self) -> usize {
        self.len()
    }

    fn safe_access(&self) -> ShmMemoryBacking {
        ShmMemoryBacking::Ptr(&**self)
    }

    fn access(&self, f: &mut dyn FnMut(&[Cell<u8>])) -> Result<(), Box<dyn Error + Sync + Send>> {
        f(self);
        Ok(())
    }
}

pub trait AsyncShmGfxTexture: GfxTexture {
    fn staging_size(&self) -> usize {
        0
    }

    fn async_upload(
        self: Rc<Self>,
        staging: &Rc<dyn GfxStagingBuffer>,
        callback: Rc<dyn AsyncShmGfxTextureCallback>,
        mem: Rc<dyn ShmMemory>,
        damage: Region,
    ) -> Result<Option<PendingShmTransfer>, GfxError>;

    fn async_upload_from_buffer(
        self: Rc<Self>,
        buf: &Rc<dyn GfxBuffer>,
        callback: Rc<dyn AsyncShmGfxTextureCallback>,
        damage: Region,
    ) -> Result<Option<PendingShmTransfer>, GfxError> {
        let _ = buf;
        let _ = callback;
        let _ = damage;

        #[derive(Debug, Error)]
        #[error("Host buffers are not supported")]
        struct E;
        Err(GfxError(Box::new(E)))
    }

    fn sync_upload(self: Rc<Self>, shm: &[Cell<u8>], damage: Region) -> Result<(), GfxError>;

    fn compatible_with(
        &self,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> bool;
}

pub trait GfxContext: Debug {
    fn reset_status(&self) -> Option<ResetStatus>;

    fn render_node(&self) -> Option<Rc<CString>>;

    fn formats(&self) -> Rc<AHashMap<u32, GfxFormat>>;

    fn fast_ram_access(&self) -> bool;

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

    fn create_internal_fb(
        self: Rc<Self>,
        cpu_worker: &Rc<CpuWorker>,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
    ) -> Result<Rc<dyn GfxInternalFramebuffer>, GfxError>;

    fn sync_obj_ctx(&self) -> Option<&Rc<SyncObjCtx>>;

    fn create_staging_buffer(
        &self,
        size: usize,
        usecase: StagingBufferUsecase,
    ) -> Rc<dyn GfxStagingBuffer> {
        let _ = usecase;
        struct Dummy(usize);
        impl GfxStagingBuffer for Dummy {
            fn size(&self) -> usize {
                self.0
            }
        }
        Rc::new(Dummy(size))
    }

    fn acquire_blend_buffer(
        &self,
        width: i32,
        height: i32,
    ) -> Result<Rc<dyn GfxBlendBuffer>, GfxError>;

    fn supports_color_management(&self) -> bool {
        false
    }

    fn supports_invalid_modifier(&self) -> bool {
        false
    }

    fn create_dmabuf_buffer(
        &self,
        dmabuf: &OwnedFd,
        offset: usize,
        size: usize,
        format: &'static Format,
    ) -> Result<Rc<dyn GfxBuffer>, GfxError> {
        let _ = dmabuf;
        let _ = offset;
        let _ = size;
        let _ = format;

        #[derive(Debug, Error)]
        #[error("Host buffers are not supported")]
        struct E;
        Err(GfxError(Box::new(E)))
    }
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
    pub supports_shm: bool,
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
            supports_shm: self.supports_shm && other.supports_shm,
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

impl PendingShmTransfer {
    pub fn new(cancel: Rc<dyn AsyncShmGfxTextureTransferCancellable>, id: u64) -> Self {
        Self { cancel, id }
    }
}

impl Drop for PendingShmTransfer {
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
    fill_black_in_grace_period: bool,
    transform: Transform,
    visualizer: Option<&DamageVisualizer>,
) -> GfxRenderPass {
    if fill_black_in_grace_period && state.idle.in_grace_period.get() {
        return GfxRenderPass {
            ops: vec![],
            clear: Some(Color::SOLID_BLACK),
            clear_cd: state.color_manager.srgb_gamma22().linear.clone(),
        };
    }
    let mut ops = vec![];
    let mut renderer = Renderer {
        base: renderer_base(physical_size, &mut ops, scale, transform),
        state,
        logical_extents: node.node_absolute_position().at_point(0, 0),
        pixel_extents: {
            let (width, height) = logical_size(physical_size, transform);
            Rect::new(0, 0, width, height).unwrap()
        },
        icons: state.icons.get(state, scale),
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
            if let Some(highlight) = seat.ui_drag_highlight() {
                renderer.render_highlight(&highlight.move_(-rect.x1(), -rect.y1()));
            }
            if let Some(drag) = seat.toplevel_drag() {
                drag.render(&mut renderer, &rect, x, y);
            }
            if let Some(dnd_icon) = seat.dnd_icon() {
                dnd_icon.render(&mut renderer, &rect, x, y);
            }
            if render_cursor {
                let cursor_user_group = seat.cursor_group();
                if (render_hardware_cursor || !cursor_user_group.hardware_cursor())
                    && let Some(cursor_user) = cursor_user_group.active()
                    && let Some(cursor) = cursor_user.get()
                {
                    cursor.tick();
                    let (mut x, mut y) = cursor_user.position();
                    x -= Fixed::from_int(rect.x1());
                    y -= Fixed::from_int(rect.y1());
                    cursor.render(&mut renderer, x, y);
                }
            }
        }
    }
    if let Some(visualizer) = visualizer
        && let Some(cursor_rect) = cursor_rect
    {
        visualizer.render(&cursor_rect, &mut renderer.base);
    }
    let c = match black_background {
        true => Color::SOLID_BLACK,
        false => state.theme.colors.background.get(),
    };
    GfxRenderPass {
        ops,
        clear: Some(c),
        clear_cd: state.color_manager.srgb_gamma22().linear.clone(),
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
