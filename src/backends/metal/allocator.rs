use {
    crate::{
        allocator::BufferObject,
        backends::metal::{
            MetalBackend, MetalError,
            video::{MetalDrmDevice, MetalRenderContext},
        },
        cmm::cmm_description::ColorDescription,
        copy_device::{CopyDevice, CopyDeviceError, CopyDeviceSupport},
        format::Format,
        gfx_api::{
            AcquireSync, GfxBlendBuffer, GfxError, GfxFormat, GfxFramebuffer, GfxTexture,
            GfxWriteModifier, ReleaseSync, SyncFile, needs_render_usage,
        },
        rect::{DamageQueue, Rect, Region},
        utils::{errorfmt::ErrorFmt, rc_eq::rc_eq},
        video::{
            LINEAR_MODIFIER, Modifier,
            dmabuf::DmaBuf,
            drm::{DrmError, DrmFramebuffer},
            gbm::{GBM_BO_USE_LINEAR, GBM_BO_USE_RENDERING, GBM_BO_USE_SCANOUT, GbmBo, GbmError},
        },
    },
    ahash::HashSet,
    arrayvec::ArrayVec,
    bstr::ByteSlice,
    indexmap::{IndexMap, IndexSet},
    isnt::std_1::primitive::IsntSliceExt,
    linearize::{Linearize, LinearizeExt, StaticMap},
    log::Level,
    run_on_drop::on_drop,
    std::{
        cell::{Cell, RefCell},
        error::Error,
        fmt::{self, Debug, Display, Formatter},
        rc::Rc,
        sync::LazyLock,
    },
    thiserror::Error,
};

#[derive(Debug)]
pub struct RenderBuffer {
    pub width: i32,
    pub height: i32,
    pub locked: Cell<bool>,
    pub format: &'static Format,
    pub drm: Rc<DrmFramebuffer>,
    pub damage_queue: DamageQueue,
    pub blend_buffer: Option<Rc<dyn GfxBlendBuffer>>,
    pub render: RenderBufferRender,
    pub dev_ctx: Rc<MetalRenderContext>,
    pub prime: RenderBufferPrime,
}

#[derive(Debug)]
pub struct RenderBufferRender {
    pub ctx: Rc<MetalRenderContext>,
    pub bo: GbmBo,
    pub tex: Rc<dyn GfxTexture>,
    pub fb: Rc<dyn GfxFramebuffer>,
}

#[derive(Debug)]
pub enum RenderBufferPrime {
    None,
    Sampling {
        dev_bo: GbmBo,
        dev_fb: Rc<dyn GfxFramebuffer>,
        // Import of the render dmabuf into the dev ctx.
        dev_render_tex: Rc<dyn GfxTexture>,
    },
}

#[derive(Debug, Error)]
pub enum RenderBufferError {
    #[error("Cannot copy between buffers of different size")]
    NotSameSize,
    #[error(transparent)]
    GfxError(#[from] GfxError),
    #[error("Could not copy frame to output device")]
    CopyToOutput(#[source] GfxError),
    #[error("Could not create a copy device copy")]
    CreateCopyDeviceCopy(#[source] CopyDeviceError),
    #[error("Could not execute a copy device copy")]
    ExecuteCopyDeviceCopy(#[source] CopyDeviceError),
}

#[derive(Default)]
pub struct RenderBufferCopy {
    pub render_block: Option<SyncFile>,
    pub present_block: Option<SyncFile>,
}

impl RenderBufferCopy {
    pub fn for_both(sf: Option<SyncFile>) -> Self {
        Self {
            render_block: sf.clone(),
            present_block: sf,
        }
    }
}

impl RenderBuffer {
    pub fn copy_to_dev(
        &self,
        cd: &Rc<ColorDescription>,
        _region: Option<&Region>,
        sync_file: Option<SyncFile>,
    ) -> Result<RenderBufferCopy, RenderBufferError> {
        match &self.prime {
            RenderBufferPrime::None => Ok(RenderBufferCopy {
                render_block: None,
                present_block: sync_file,
            }),
            RenderBufferPrime::Sampling {
                dev_render_tex,
                dev_fb,
                ..
            } => dev_fb
                .copy_texture(
                    AcquireSync::Unnecessary,
                    ReleaseSync::Explicit,
                    cd,
                    dev_render_tex,
                    cd,
                    None,
                    AcquireSync::from_sync_file(sync_file),
                    ReleaseSync::None,
                    0,
                    0,
                )
                .map_err(RenderBufferError::CopyToOutput)
                .map(RenderBufferCopy::for_both),
        }
    }

    pub fn damage_full(&self) {
        let rect = Rect::new_sized_saturating(0, 0, self.width, self.height);
        self.damage_queue.clear_all();
        self.damage_queue.damage(&[rect]);
    }

    pub fn clear(&self, cd: &Rc<ColorDescription>) -> Result<Option<SyncFile>, RenderBufferError> {
        match &self.prime {
            RenderBufferPrime::None => self
                .render
                .fb
                .clear(AcquireSync::Unnecessary, ReleaseSync::Explicit, cd)
                .map_err(Into::into),
            RenderBufferPrime::Sampling { dev_fb, .. } => dev_fb
                .clear(AcquireSync::Unnecessary, ReleaseSync::Explicit, cd)
                .map_err(Into::into),
        }
    }

    pub fn copy_to_new(
        &self,
        new: &Self,
        cd: &Rc<ColorDescription>,
    ) -> Result<Option<SyncFile>, RenderBufferError> {
        let old = self;

        if (old.width, old.height) != (new.width, new.height) {
            return Err(RenderBufferError::NotSameSize);
        }

        if let Some(dev) = new.dev_copy_device().or(old.dev_copy_device()) {
            return dev
                .create_copy(old.dev_bo().dmabuf(), new.dev_bo().dmabuf())
                .map_err(RenderBufferError::CreateCopyDeviceCopy)?
                .execute(None, None)
                .map_err(RenderBufferError::ExecuteCopyDeviceCopy);
        }

        let copy_texture_impl = |fb: &Rc<dyn GfxFramebuffer>, tex: &Rc<dyn GfxTexture>| {
            fb.copy_texture(
                AcquireSync::Unnecessary,
                ReleaseSync::Explicit,
                cd,
                tex,
                cd,
                None,
                AcquireSync::Unnecessary,
                ReleaseSync::Explicit,
                0,
                0,
            )
        };
        let copy_texture = |new_ctx: &Rc<MetalRenderContext>,
                            fb: &Rc<dyn GfxFramebuffer>,
                            old_ctx: &Rc<MetalRenderContext>,
                            tex: &Rc<dyn GfxTexture>,
                            dma_buf: &DmaBuf| {
            if rc_eq(&new_ctx.gfx, &old_ctx.gfx) {
                return copy_texture_impl(fb, tex);
            }
            let tex = new_ctx.gfx.clone().dmabuf_img(dma_buf)?.to_texture()?;
            copy_texture_impl(fb, &tex)
        };

        let sf = match &old.prime {
            RenderBufferPrime::None => match &new.prime {
                RenderBufferPrime::None => copy_texture(
                    &new.render.ctx,
                    &new.render.fb,
                    &old.render.ctx,
                    &old.render.tex,
                    old.render.bo.dmabuf(),
                )?,
                RenderBufferPrime::Sampling {
                    dev_fb: new_dev_fb, ..
                } => copy_texture(
                    &new.dev_ctx,
                    new_dev_fb,
                    &old.render.ctx,
                    &old.render.tex,
                    old.render.bo.dmabuf(),
                )?,
            },
            RenderBufferPrime::Sampling {
                dev_render_tex: old_dev_render_tex,
                dev_bo: old_dev_bo,
                ..
            } => match &new.prime {
                RenderBufferPrime::None => copy_texture(
                    &new.render.ctx,
                    &new.render.fb,
                    &old.dev_ctx,
                    old_dev_render_tex,
                    old_dev_bo.dmabuf(),
                )?,
                RenderBufferPrime::Sampling {
                    dev_fb: new_dev_fb, ..
                } => copy_texture(
                    &new.dev_ctx,
                    &new_dev_fb,
                    &old.dev_ctx,
                    old_dev_render_tex,
                    old_dev_bo.dmabuf(),
                )?,
            },
        };
        Ok(sf)
    }

    pub fn dev_bo(&self) -> &GbmBo {
        match &self.prime {
            RenderBufferPrime::None => &self.render.bo,
            RenderBufferPrime::Sampling { dev_bo, .. } => dev_bo,
        }
    }

    pub fn dev_copy_device(&self) -> Option<&Rc<CopyDevice>> {
        match &self.prime {
            RenderBufferPrime::None => None,
            RenderBufferPrime::Sampling { .. } => None,
        }
    }
}

struct Builder<'a> {
    slf: &'a MetalBackend,
    dev: &'a Rc<MetalDrmDevice>,
    dev_ctx: &'a Rc<MetalRenderContext>,
    format: &'static Format,
    render_fmt: &'a GfxFormat,
    plane_modifiers: &'a IndexSet<Modifier>,
    width: i32,
    height: i32,
    render_ctx: &'a Rc<MetalRenderContext>,
    cursor: bool,
    blend_buffer: Option<&'a Rc<dyn GfxBlendBuffer>>,
}

struct BoAllocationSettings {
    modifiers: Vec<Modifier>,
    usage: u32,
}

struct NoPrime {
    allocation_settings: BoAllocationSettings,
}

struct PrimeSampling {
    render_allocation_settings: BoAllocationSettings,
    dev_allocation_settings: BoAllocationSettings,
}

impl MetalBackend {
    pub fn create_scanout_buffers<const N: usize>(
        &self,
        dev: &Rc<MetalDrmDevice>,
        format: &'static Format,
        plane_modifiers: &IndexSet<Modifier>,
        width: i32,
        height: i32,
        render_ctx: &Rc<MetalRenderContext>,
        cursor: bool,
    ) -> Result<[RenderBuffer; N], MetalError> {
        let Some(render_fmt) = render_ctx.gfx.formats().get(&format.drm) else {
            return Err(MetalError::RenderUnsupportedFormat);
        };
        let mut blend_buffer = None;
        if !cursor {
            match render_ctx.gfx.acquire_blend_buffer(width, height) {
                Ok(bb) => blend_buffer = Some(bb),
                Err(e) => {
                    log::warn!("Could not create blend buffer: {}", ErrorFmt(e));
                }
            }
        }
        let builder = Builder {
            slf: self,
            dev,
            dev_ctx: &dev.ctx.get(),
            format,
            render_fmt,
            plane_modifiers,
            width,
            height,
            render_ctx,
            cursor,
            blend_buffer: blend_buffer.as_ref(),
        };
        if render_ctx.dev_id == dev.id {
            return wrap_error(&builder, None, |dbg| {
                let prepared = &builder.prepare_prime_none(dbg)?;
                self.create_scanout_buffers_(|damage| {
                    builder.create_prime_none(prepared, damage, dbg)
                })
            })
            .map_err(MetalError::AllocateScanoutBuffer);
        }
        let mut errors = ScanoutBufferErrors::default();
        for &method in &*PRIME_METHODS {
            let res = wrap_error(&builder, Some(method), |dbg| {
                macro_rules! x {
                    ($prepare:ident, $create:ident $(,)?) => {{
                        let prepared = &builder.$prepare(dbg)?;
                        self.create_scanout_buffers_(|damage| {
                            builder.$create(prepared, damage, dbg)
                        })
                    }};
                }
                match method {
                    PrimeMethod::Sampling => {
                        x!(prepare_prime_sampling, create_prime_sampling)
                    }
                }
            });
            match res {
                Err(e) => errors.errors.push(e),
                Ok(b) => {
                    if errors.errors.is_not_empty() {
                        log::warn!("Preferred prime methods failed");
                        let debug = log::log_enabled!(Level::Debug);
                        for error in &errors.errors {
                            let Some(method) = error.prime else {
                                continue;
                            };
                            if debug {
                                log::warn!("- {method}: {}", ErrorFmt(error));
                            } else {
                                log::warn!("- {method}: {}", ErrorFmt(&error.kind));
                            }
                        }
                    }
                    return Ok(b);
                }
            }
        }
        Err(MetalError::AllocateScanoutBufferPrime(errors))
    }

    fn create_scanout_buffers_<const N: usize>(
        &self,
        allocate: impl Fn(DamageQueue) -> Result<RenderBuffer, ScanoutBufferErrorKind>,
    ) -> Result<[RenderBuffer; N], ScanoutBufferErrorKind> {
        let mut damage_queue = ArrayVec::from(DamageQueue::new::<N>());
        let mut array = ArrayVec::<_, N>::new();
        for _ in 0..N {
            let damage_queue = damage_queue.pop().unwrap();
            array.push(allocate(damage_queue)?);
        }
        if let Some(buffer) = array.first() {
            buffer.damage_full();
        }
        Ok(array.into_inner().unwrap())
    }
}

#[derive(Debug, Error)]
pub enum ScanoutBufferErrorKind {
    #[error("Scanout device: The format is not supported")]
    SodUnsupportedFormat,
    #[error("Scanout device: Buffer allocation failed")]
    SodBufferAllocation(#[source] GbmError),
    #[error("Scanout device: addfb2 failed")]
    SodAddfb2(#[source] DrmError),
    #[error("Scanout device: Could not import SCANOUT buffer into the gfx API")]
    SodImportSodImage(#[source] GfxError),
    #[error("Scanout device: Could not turn imported SCANOUT buffer into gfx API FB")]
    SodImportFb(#[source] GfxError),
    #[error("Render device: The intersection of render/sample/sod_sample modifiers is empty")]
    RenderWriteReadSodReadIntersection,
    #[error("Scanout device: The intersection of render/sample/plane modifiers is empty")]
    SodWriteReadPlaneIntersection,
    #[error("Scanout device: The intersection of render/plane modifiers is empty")]
    SodWritePlaneIntersection,
    #[error("Render device: Buffer allocation failed")]
    RenderBufferAllocation(#[source] GbmError),
    #[error("Render device: Could not import RENDER buffer into the gfx API")]
    RenderImportImage(#[source] GfxError),
    #[error("Render device: Could not turn imported RENDER buffer into gfx API FB")]
    RenderImportFb(#[source] GfxError),
    #[error("Render device: Could not clear RENDER buffer")]
    RenderClear(#[source] GfxError),
    #[error("Render device: Could not turn imported RENDER buffer into gfx API texture")]
    RenderImportRenderTexture(#[source] GfxError),
    #[error("Scanout device: Could not import RENDER buffer into the gfx API")]
    SodImportRenderImage(#[source] GfxError),
    #[error("Scanout device: Could not turn imported RENDER buffer into gfx API texture")]
    SodImportRenderTexture(#[source] GfxError),
}

#[derive(Default, Debug)]
pub struct ScanoutBufferErrors {
    #[expect(clippy::vec_box)]
    errors: Vec<Box<ScanoutBufferError>>,
}

#[derive(Debug)]
pub struct ScanoutBufferError {
    dev: String,
    render_name: Option<String>,
    format: &'static Format,
    plane_modifiers: IndexSet<Modifier>,
    width: i32,
    height: i32,
    cursor: bool,
    dbg: RenderBufferAllocationDebug,
    kind: ScanoutBufferErrorKind,
    prime: Option<PrimeMethod>,
}

#[derive(Copy, Clone, Linearize)]
pub enum PrimeMethod {
    Sampling,
}

impl PrimeMethod {
    pub fn name(self) -> &'static str {
        match self {
            PrimeMethod::Sampling => "direct-sampling",
        }
    }
}

impl Display for PrimeMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Debug for PrimeMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl RenderBufferPrime {
    pub fn method(&self) -> Option<PrimeMethod> {
        let method = match self {
            RenderBufferPrime::None => return None,
            RenderBufferPrime::Sampling { .. } => PrimeMethod::Sampling,
        };
        Some(method)
    }
}

#[derive(Default, Debug)]
struct RenderBufferAllocationDebug {
    dev_copy_src_modifiers: Option<Vec<Modifier>>,
    dev_copy_dst_modifiers: Option<Vec<Modifier>>,
    dev_gfx_write_modifiers: Option<Vec<Modifier>>,
    dev_gfx_read_modifiers: Option<Vec<Modifier>>,
    dev_modifiers_possible: Option<Vec<Modifier>>,
    dev_usage: Option<u32>,
    dev_modifier: Option<Modifier>,
    render_copy_src_modifiers: Option<Vec<Modifier>>,
    render_copy_dst_modifiers: Option<Vec<Modifier>>,
    render_gfx_write_modifiers: Option<Vec<Modifier>>,
    render_gfx_read_modifiers: Option<Vec<Modifier>>,
    render_modifiers_possible: Option<Vec<Modifier>>,
    render_usage: Option<u32>,
    render_modifier: Option<Modifier>,
}

impl Display for ScanoutBufferErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (idx, error) in self.errors.iter().enumerate() {
            if idx > 0 {
                writeln!(f, "\n------")?;
            }
            write!(f, "{}", ErrorFmt(error))?;
        }
        Ok(())
    }
}

impl Error for ScanoutBufferErrors {}

impl Display for ScanoutBufferError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        if let Some(v) = self.prime {
            writeln!(f, "prime type: {}", v)?;
        }
        writeln!(f, "scanout device: {}", self.dev)?;
        writeln!(f, "format: {}", self.format.name)?;
        writeln!(f, "plane modifiers: {:x?}", self.plane_modifiers)?;
        writeln!(f, "size: {}x{}", self.width, self.height)?;
        writeln!(f, "cursor: {}", self.cursor)?;
        if let Some(v) = &self.dbg.dev_copy_src_modifiers {
            writeln!(f, "scanout copy src modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.dev_copy_dst_modifiers {
            writeln!(f, "scanout copy dst modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.dev_gfx_write_modifiers {
            writeln!(f, "scanout gfx writable modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.dev_modifiers_possible {
            writeln!(f, "scanout dev possible modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.dev_usage {
            writeln!(f, "scanout dev gbm usage: {:x}", v)?;
        }
        if let Some(v) = &self.dbg.dev_modifier {
            writeln!(f, "scanout dev modifier: {:x}", v)?;
        }
        if let Some(v) = &self.render_name {
            writeln!(f, "render device: {}", v)?;
        }
        if let Some(v) = &self.dbg.render_copy_src_modifiers {
            writeln!(f, "render copy src modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.render_copy_dst_modifiers {
            writeln!(f, "render copy dst modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.render_gfx_write_modifiers {
            writeln!(f, "render gfx writable modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.render_gfx_read_modifiers {
            writeln!(f, "render gfx readable modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.dev_gfx_read_modifiers {
            writeln!(f, "scanout gfx readable modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.render_modifiers_possible {
            writeln!(f, "render dev possible modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dbg.render_usage {
            writeln!(f, "render dev gbm usage: {:x}", v)?;
        }
        if let Some(v) = &self.dbg.render_modifier {
            writeln!(f, "render dev modifier: {:x}", v)?;
        }
        Ok(())
    }
}

impl Error for ScanoutBufferError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.kind)
    }
}

fn wrap_error<T>(
    common: &Builder<'_>,
    prime: Option<PrimeMethod>,
    f: impl FnOnce(&RefCell<RenderBufferAllocationDebug>) -> Result<T, ScanoutBufferErrorKind>,
) -> Result<T, Box<ScanoutBufferError>> {
    let dbg = Default::default();
    f(&dbg)
        .map_err(|kind| ScanoutBufferError {
            dev: common.dev.devnode.as_bytes().as_bstr().to_string(),
            render_name: (common.dev.id != common.render_ctx.dev_id)
                .then(|| common.render_ctx.devnode.as_bytes().as_bstr().to_string()),
            format: common.format,
            plane_modifiers: common.plane_modifiers.clone(),
            width: common.width,
            height: common.height,
            cursor: common.cursor,
            dbg: dbg.into_inner(),
            kind,
            prime,
        })
        .map_err(Box::new)
}

impl BoAllocationSettings {
    fn new1(
        common: &Builder<'_>,
        modifiers: &IndexMap<Modifier, &GfxWriteModifier>,
        scanout: bool,
        rendering: bool,
        usage_out: &mut Option<u32>,
    ) -> Self {
        let needs_render_usage = rendering && needs_render_usage(modifiers.values().copied());
        Self::new3(
            common,
            modifiers.keys(),
            scanout,
            needs_render_usage,
            usage_out,
        )
    }

    #[expect(dead_code)]
    fn new2<'a>(
        common: &Builder<'_>,
        modifiers: impl IntoIterator<Item = &'a Modifier> + Clone,
        fmt: &GfxFormat,
        scanout: bool,
        rendering: bool,
        usage_out: &mut Option<u32>,
    ) -> Self {
        let needs_render_usage = rendering
            && needs_render_usage(
                modifiers
                    .clone()
                    .into_iter()
                    .filter_map(|m| fmt.write_modifiers.get(m)),
            );
        Self::new3(common, modifiers, scanout, needs_render_usage, usage_out)
    }

    fn new3<'a>(
        common: &Builder<'_>,
        modifiers: impl IntoIterator<Item = &'a Modifier>,
        scanout: bool,
        needs_render_usage: bool,
        usage_out: &mut Option<u32>,
    ) -> Self {
        let mut usage = 0;
        if scanout {
            usage |= GBM_BO_USE_SCANOUT;
            if common.cursor {
                usage |= GBM_BO_USE_LINEAR;
            }
        }
        if needs_render_usage {
            usage |= GBM_BO_USE_RENDERING;
        }
        *usage_out = Some(usage);
        Self {
            modifiers: modifiers.into_iter().copied().collect(),
            usage,
        }
    }
}

impl Builder<'_> {
    fn create(
        &self,
        drm: Rc<DrmFramebuffer>,
        damage_queue: DamageQueue,
        render: RenderBufferRender,
        prime: RenderBufferPrime,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let Self {
            dev_ctx,
            format,
            width,
            height,
            blend_buffer,
            ..
        } = *self;
        Ok(RenderBuffer {
            width,
            height,
            locked: Cell::new(true),
            format,
            drm,
            damage_queue,
            blend_buffer: blend_buffer.cloned(),
            render,
            dev_ctx: dev_ctx.clone(),
            prime,
        })
    }

    fn create_bo(
        &self,
        ctx: &MetalRenderContext,
        settings: &BoAllocationSettings,
    ) -> Result<GbmBo, GbmError> {
        ctx.gbm.create_bo(
            &self.slf.state.dma_buf_ids,
            self.width,
            self.height,
            self.format,
            &settings.modifiers,
            settings.usage,
        )
    }

    fn create_dev_bo(
        &self,
        settings: &BoAllocationSettings,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<(GbmBo, Rc<DrmFramebuffer>), ScanoutBufferErrorKind> {
        let bo = self
            .create_bo(self.dev_ctx, settings)
            .map_err(ScanoutBufferErrorKind::SodBufferAllocation)?;
        let send_dev_modifier = on_drop(|| {
            dbg.borrow_mut().dev_modifier = Some(bo.dmabuf().modifier);
        });
        let drm = self
            .dev
            .master
            .add_fb(bo.dmabuf(), None)
            .map(Rc::new)
            .map_err(ScanoutBufferErrorKind::SodAddfb2)?;
        send_dev_modifier.forget();
        Ok((bo, drm))
    }

    fn create_render_buffer_render(
        &self,
        settings: &BoAllocationSettings,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBufferRender, ScanoutBufferErrorKind> {
        let Self { render_ctx, .. } = *self;
        let bo = self
            .create_bo(render_ctx, settings)
            .map_err(ScanoutBufferErrorKind::RenderBufferAllocation)?;
        let send_render_modifier = on_drop(|| {
            dbg.borrow_mut().render_modifier = Some(bo.dmabuf().modifier);
        });
        let img = render_ctx
            .gfx
            .clone()
            .dmabuf_img(bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::RenderImportImage)?;
        let tex = img
            .clone()
            .to_texture()
            .map_err(ScanoutBufferErrorKind::RenderImportRenderTexture)?;
        let fb = img
            .to_framebuffer()
            .map_err(ScanoutBufferErrorKind::RenderImportFb)?;
        fb.clear(
            AcquireSync::Unnecessary,
            ReleaseSync::None,
            self.slf.state.color_manager.srgb_gamma22(),
        )
        .map_err(ScanoutBufferErrorKind::RenderClear)?;
        send_render_modifier.forget();
        Ok(RenderBufferRender {
            ctx: render_ctx.clone(),
            bo,
            tex,
            fb,
        })
    }

    fn copy_modifiers_iter(&self, support: &[CopyDeviceSupport]) -> impl Iterator<Item = Modifier> {
        let Builder { width, height, .. } = *self;
        support
            .iter()
            .filter(move |s| s.max_width >= width as _ && s.max_height >= height as _)
            .map(move |s| s.modifier)
    }

    fn copy_modifiers(&self, support: &[CopyDeviceSupport]) -> Vec<Modifier> {
        self.copy_modifiers_iter(support).collect()
    }

    #[expect(dead_code)]
    fn copy_src_modifiers(&self, dev: &CopyDevice) -> Vec<Modifier> {
        self.copy_modifiers(dev.src_support(self.format))
    }

    #[expect(dead_code)]
    fn copy_dst_modifiers(&self, dev: &CopyDevice) -> Vec<Modifier> {
        self.copy_modifiers(dev.dst_support(self.format))
    }

    fn copy_supports_linear(&self, support: &[CopyDeviceSupport]) -> bool {
        self.copy_modifiers_iter(support)
            .any(|m| m == LINEAR_MODIFIER)
    }

    #[expect(dead_code)]
    fn copy_src_supports_linear(&self, dev: &CopyDevice) -> bool {
        self.copy_supports_linear(dev.src_support(self.format))
    }

    #[expect(dead_code)]
    fn copy_dst_supports_linear(&self, dev: &CopyDevice) -> bool {
        self.copy_supports_linear(dev.dst_support(self.format))
    }

    fn prepare_prime_none(
        &self,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<NoPrime, ScanoutBufferErrorKind> {
        let dbg = &mut *dbg.borrow_mut();
        let Self {
            render_fmt,
            plane_modifiers,
            ..
        } = *self;
        let modifiers: IndexMap<_, _> = render_fmt
            .write_modifiers
            .iter()
            .map(|(m, v)| (*m, v))
            .filter(|(m, _)| plane_modifiers.contains(m))
            .filter(|(m, _)| render_fmt.read_modifiers.contains(m))
            .collect();
        dbg.dev_gfx_write_modifiers = Some(render_modifiers(render_fmt));
        dbg.dev_gfx_read_modifiers = Some(sample_modifiers(render_fmt));
        dbg.dev_modifiers_possible = Some(modifiers.keys().copied().collect());
        if modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::SodWriteReadPlaneIntersection);
        }
        let allocation_settings =
            BoAllocationSettings::new1(self, &modifiers, true, true, &mut dbg.render_usage);
        Ok(NoPrime {
            allocation_settings,
        })
    }

    fn create_prime_none(
        &self,
        prepared: &NoPrime,
        damage_queue: DamageQueue,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let NoPrime {
            allocation_settings,
        } = prepared;
        let Self { dev, .. } = *self;
        let render = self.create_render_buffer_render(allocation_settings, dbg)?;
        let send_dev_modifier = on_drop(|| {
            dbg.borrow_mut().dev_modifier = Some(render.bo.dmabuf().modifier);
        });
        let drm = dev
            .master
            .add_fb(render.bo.dmabuf(), None)
            .map(Rc::new)
            .map_err(ScanoutBufferErrorKind::SodAddfb2)?;
        send_dev_modifier.forget();
        let prime = RenderBufferPrime::None;
        self.create(drm, damage_queue, render, prime)
    }

    fn prepare_prime_sampling(
        &self,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<PrimeSampling, ScanoutBufferErrorKind> {
        let dbg = &mut *dbg.borrow_mut();
        let Self {
            dev_ctx,
            format,
            render_fmt,
            plane_modifiers,
            ..
        } = *self;
        let Some(dev_fmt) = dev_ctx.gfx.formats().get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::SodUnsupportedFormat);
        };
        let render_modifiers_possible: IndexMap<_, _> = render_fmt
            .write_modifiers
            .iter()
            .filter(|(m, _)| render_fmt.read_modifiers.contains(*m))
            .filter(|(m, _)| dev_fmt.read_modifiers.contains(*m))
            .map(|(m, v)| (*m, v))
            .collect();
        dbg.dev_gfx_read_modifiers = Some(sample_modifiers(dev_fmt));
        dbg.render_gfx_write_modifiers = Some(render_modifiers(render_fmt));
        dbg.render_gfx_read_modifiers = Some(sample_modifiers(render_fmt));
        dbg.render_modifiers_possible = Some(render_modifiers_possible.keys().copied().collect());
        if render_modifiers_possible.is_empty() {
            return Err(ScanoutBufferErrorKind::RenderWriteReadSodReadIntersection);
        }
        let dev_modifiers_possible: IndexMap<_, _> = dev_fmt
            .write_modifiers
            .iter()
            .filter(|(m, _)| plane_modifiers.contains(*m))
            .map(|(m, v)| (*m, v))
            .collect();
        dbg.dev_gfx_write_modifiers = Some(render_modifiers(dev_fmt));
        dbg.dev_modifiers_possible = Some(dev_modifiers_possible.keys().copied().collect());
        if dev_modifiers_possible.is_empty() {
            return Err(ScanoutBufferErrorKind::SodWritePlaneIntersection);
        }
        let render_allocation_settings = BoAllocationSettings::new1(
            self,
            &render_modifiers_possible,
            false,
            true,
            &mut dbg.render_usage,
        );
        let dev_allocation_settings = BoAllocationSettings::new1(
            self,
            &dev_modifiers_possible,
            true,
            true,
            &mut dbg.dev_usage,
        );
        Ok(PrimeSampling {
            render_allocation_settings,
            dev_allocation_settings,
        })
    }

    fn create_prime_sampling(
        &self,
        prepared: &PrimeSampling,
        damage_queue: DamageQueue,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let PrimeSampling {
            render_allocation_settings,
            dev_allocation_settings,
        } = prepared;
        let Self { dev_ctx, .. } = *self;
        let render = self.create_render_buffer_render(render_allocation_settings, dbg)?;
        let send_render_modifier = on_drop(|| {
            dbg.borrow_mut().render_modifier = Some(render.bo.dmabuf().modifier);
        });
        let dev_render_tex = dev_ctx
            .gfx
            .clone()
            .dmabuf_img(render.bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::SodImportRenderImage)?
            .to_texture()
            .map_err(ScanoutBufferErrorKind::SodImportRenderTexture)?;
        let (dev_bo, drm) = self.create_dev_bo(dev_allocation_settings, dbg)?;
        let send_dev_modifier = on_drop(|| {
            dbg.borrow_mut().dev_modifier = Some(dev_bo.dmabuf().modifier);
        });
        let dev_fb = dev_ctx
            .gfx
            .clone()
            .dmabuf_img(dev_bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::SodImportSodImage)?
            .to_framebuffer()
            .map_err(ScanoutBufferErrorKind::SodImportFb)?;
        send_dev_modifier.forget();
        send_render_modifier.forget();
        let prime = RenderBufferPrime::Sampling {
            dev_bo,
            dev_fb,
            dev_render_tex,
        };
        self.create(drm, damage_queue, render, prime)
    }
}

const JAY_PRIME_METHODS: &str = "JAY_PRIME_METHODS";

type PrimeMethods = ArrayVec<PrimeMethod, { PrimeMethod::LENGTH }>;

static PRIME_METHODS: LazyLock<PrimeMethods> = LazyLock::new(prime_methods);

fn prime_methods() -> PrimeMethods {
    let mut res = PrimeMethods::new();
    let mut seen = StaticMap::<_, bool>::default();
    let mut apply = |method: PrimeMethod, allow: bool| {
        if !seen[method] {
            seen[method] = true;
            if allow {
                res.push(method);
            }
        }
    };
    if let Ok(var) = std::env::var(JAY_PRIME_METHODS) {
        for mut name in var.split(",") {
            name = name.trim();
            if name.is_empty() {
                continue;
            }
            let mut allow = true;
            if let Some(m) = name.strip_prefix("-") {
                name = m;
                allow = false;
            }
            let Some(method) = PrimeMethod::variants().find(|m| m.name() == name) else {
                log::warn!("Unknown prime method {}", name);
                continue;
            };
            apply(method, allow);
        }
    }
    for method in PrimeMethod::variants() {
        apply(method, true);
    }
    log::info!("Prime methods: {:?}", res);
    res
}

fn sample_modifiers(fmt: &GfxFormat) -> Vec<Modifier> {
    fmt.read_modifiers.iter().copied().collect()
}

fn render_modifiers(fmt: &GfxFormat) -> Vec<Modifier> {
    fmt.write_modifiers.keys().copied().collect()
}

fn intersect_modifiers<'a>(
    left: impl IntoIterator<Item = &'a Modifier>,
    right: impl IntoIterator<Item = &'a Modifier>,
) -> Vec<Modifier> {
    let right: HashSet<_> = right.into_iter().copied().collect();
    left.into_iter()
        .copied()
        .filter(|m| right.contains(m))
        .collect()
}

#[expect(dead_code)]
fn intersect_render_modifiers<'a>(
    left: &'a GfxFormat,
    right: impl IntoIterator<Item = &'a Modifier>,
) -> Vec<Modifier> {
    intersect_modifiers(
        left.write_modifiers
            .keys()
            .filter(|m| left.read_modifiers.contains(*m)),
        right,
    )
}

#[expect(dead_code)]
fn make_linear_only(modifiers: &mut Vec<Modifier>) {
    if modifiers.contains(&LINEAR_MODIFIER) {
        *modifiers = vec![LINEAR_MODIFIER];
    }
}
