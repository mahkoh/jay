use {
    crate::{
        allocator::BufferObject,
        backends::metal::{
            MetalBackend, MetalError,
            video::{MetalDrmDevice, MetalRenderContext},
        },
        cmm::cmm_description::ColorDescription,
        copy_device::{
            CopyDevice, CopyDeviceBuffer, CopyDeviceCopy, CopyDeviceError, CopyDeviceSupport,
        },
        format::Format,
        gfx_api::{
            AcquireSync, GfxBlendBuffer, GfxError, GfxFormat, GfxFramebuffer, GfxTexture,
            GfxWriteModifier, ReleaseSync, SyncFile, needs_render_usage,
        },
        rect::{DamageQueue, Rect, Region},
        udmabuf::{Udmabuf, UdmabufError},
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
        dev_render_tex: Rc<dyn GfxTexture>,
    },
    CopyUdmabuf {
        render_copy: CopyDeviceCopy,
        dev_copy_dev: Rc<CopyDevice>,
        dev_copy: CopyDeviceCopy,
        dev_bo: GbmBo,
    },
    CopyDirectPull {
        dev_copy_dev: Rc<CopyDevice>,
        dev_copy: CopyDeviceCopy,
        dev_bo: GbmBo,
    },
    CopyIndirectPull {
        render_copy: CopyDeviceCopy,
        _render_secondary_bo: CopyDeviceBuffer,
        dev_copy_dev: Rc<CopyDevice>,
        dev_copy: CopyDeviceCopy,
        dev_bo: GbmBo,
    },
    CopyDirectPush {
        render_copy: CopyDeviceCopy,
        dev_copy_dev: Rc<CopyDevice>,
        dev_bo: GbmBo,
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
    #[error("Could not copy render bo to udmabuf")]
    CopyRenderToUdmabuf(#[source] CopyDeviceError),
    #[error("Could not copy udmabuf to dev bo")]
    CopyUdmabufToDev(#[source] CopyDeviceError),
    #[error("Could not create a copy device copy")]
    CreateCopyDeviceCopy(#[source] CopyDeviceError),
    #[error("Could not execute a copy device copy")]
    ExecuteCopyDeviceCopy(#[source] CopyDeviceError),
    #[error("Could not copy render bo to dev bo")]
    CopyRenderToDev(#[source] CopyDeviceError),
}

pub struct RenderBufferCopy {
    pub render_sync_file: Option<SyncFile>,
    pub scanout_sync_file: Option<SyncFile>,
}

impl RenderBuffer {
    pub fn copy_to_dev(
        &self,
        cd: &Rc<ColorDescription>,
        region: Option<&Region>,
        sync_file: Option<SyncFile>,
    ) -> Result<RenderBufferCopy, RenderBufferError> {
        let mut render_sync_file = sync_file;
        let scanout_sync_file;
        match &self.prime {
            RenderBufferPrime::None => {
                scanout_sync_file = render_sync_file.take();
            }
            RenderBufferPrime::Sampling {
                dev_render_tex,
                dev_fb,
                ..
            } => {
                render_sync_file = dev_fb
                    .copy_texture(
                        AcquireSync::Unnecessary,
                        ReleaseSync::Explicit,
                        cd,
                        dev_render_tex,
                        cd,
                        None,
                        AcquireSync::from_sync_file(render_sync_file),
                        ReleaseSync::None,
                        0,
                        0,
                    )
                    .map_err(RenderBufferError::CopyToOutput)?;
                scanout_sync_file = render_sync_file.clone();
            }
            RenderBufferPrime::CopyUdmabuf {
                render_copy,
                dev_copy,
                ..
            }
            | RenderBufferPrime::CopyIndirectPull {
                render_copy,
                dev_copy,
                ..
            } => {
                render_sync_file = render_copy
                    .execute(render_sync_file.as_ref(), region)
                    .map_err(RenderBufferError::CopyRenderToUdmabuf)?;
                scanout_sync_file = dev_copy
                    .execute(render_sync_file.as_ref(), region)
                    .map_err(RenderBufferError::CopyUdmabufToDev)?;
            }
            RenderBufferPrime::CopyDirectPull { dev_copy: copy, .. }
            | RenderBufferPrime::CopyDirectPush {
                render_copy: copy, ..
            } => {
                render_sync_file = copy
                    .execute(render_sync_file.as_ref(), region)
                    .map_err(RenderBufferError::CopyRenderToDev)?;
                scanout_sync_file = render_sync_file.clone();
            }
        }
        Ok(RenderBufferCopy {
            render_sync_file,
            scanout_sync_file,
        })
    }

    pub fn damage_full(&self) {
        let rect = Rect::new_sized_saturating(0, 0, self.width, self.height);
        self.damage_queue.clear_all();
        self.damage_queue.damage(&[rect]);
    }

    pub fn clear(&self, cd: &Rc<ColorDescription>) -> Result<Option<SyncFile>, RenderBufferError> {
        let sync_file = match &self.prime {
            RenderBufferPrime::None => {
                self.render
                    .fb
                    .clear(AcquireSync::Unnecessary, ReleaseSync::Explicit, cd)?
            }
            RenderBufferPrime::Sampling { dev_fb, .. } => {
                dev_fb.clear(AcquireSync::Unnecessary, ReleaseSync::Explicit, cd)?
            }
            RenderBufferPrime::CopyUdmabuf { .. }
            | RenderBufferPrime::CopyDirectPull { .. }
            | RenderBufferPrime::CopyIndirectPull { .. }
            | RenderBufferPrime::CopyDirectPush { .. } => {
                let sf =
                    self.render
                        .fb
                        .clear(AcquireSync::Unnecessary, ReleaseSync::Explicit, cd)?;
                let sf = self.copy_to_dev(cd, None, sf)?;
                sf.scanout_sync_file
            }
        };
        Ok(sync_file)
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

        if let Some(dev) = old.dev_copy_device().or(new.dev_copy_device()) {
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
                _ => unreachable!(),
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
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        Ok(sf)
    }

    pub fn dev_bo(&self) -> &GbmBo {
        match &self.prime {
            RenderBufferPrime::None => &self.render.bo,
            RenderBufferPrime::Sampling { dev_bo, .. } => dev_bo,
            RenderBufferPrime::CopyUdmabuf { dev_bo, .. } => dev_bo,
            RenderBufferPrime::CopyDirectPull { dev_bo, .. } => dev_bo,
            RenderBufferPrime::CopyDirectPush { dev_bo, .. } => dev_bo,
            RenderBufferPrime::CopyIndirectPull { dev_bo, .. } => dev_bo,
        }
    }

    pub fn dev_copy_device(&self) -> Option<&Rc<CopyDevice>> {
        match &self.prime {
            RenderBufferPrime::None => None,
            RenderBufferPrime::Sampling { .. } => None,
            RenderBufferPrime::CopyUdmabuf { dev_copy_dev, .. }
            | RenderBufferPrime::CopyDirectPull { dev_copy_dev, .. }
            | RenderBufferPrime::CopyIndirectPull { dev_copy_dev, .. }
            | RenderBufferPrime::CopyDirectPush { dev_copy_dev, .. } => Some(dev_copy_dev),
        }
    }
}

struct CommonArgs<'a> {
    dev: &'a Rc<MetalDrmDevice>,
    dev_ctx: &'a Rc<MetalRenderContext>,
    format: &'static Format,
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

struct NoPrime<'a> {
    common: &'a CommonArgs<'a>,
    allocation_settings: BoAllocationSettings,
}

struct PrimeSampling<'a> {
    common: &'a CommonArgs<'a>,
    render_allocation_settings: BoAllocationSettings,
    dev_allocation_settings: BoAllocationSettings,
}

struct DirectCopyPull<'a> {
    common: &'a CommonArgs<'a>,
    dev_copy_dev: Rc<CopyDevice>,
    render_allocation_settings: BoAllocationSettings,
    dev_allocation_settings: BoAllocationSettings,
}

struct DirectCopyPush<'a> {
    common: &'a CommonArgs<'a>,
    render_copy_dev: Rc<CopyDevice>,
    dev_copy_dev: Rc<CopyDevice>,
    render_allocation_settings: BoAllocationSettings,
    dev_allocation_settings: BoAllocationSettings,
}

struct CopyUdmabuf<'a> {
    common: &'a CommonArgs<'a>,
    udmabuf: Rc<Udmabuf>,
    render_allocation_settings: BoAllocationSettings,
    render_copy_dev: Rc<CopyDevice>,
    dev_copy_dev: Rc<CopyDevice>,
    dev_allocation_settings: BoAllocationSettings,
}

struct IndirectCopyPull<'a> {
    common: &'a CommonArgs<'a>,
    render_allocation_settings: BoAllocationSettings,
    render_copy_dev: Rc<CopyDevice>,
    dev_copy_dev: Rc<CopyDevice>,
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
        let mut blend_buffer = None;
        if !cursor {
            match render_ctx.gfx.acquire_blend_buffer(width, height) {
                Ok(bb) => blend_buffer = Some(bb),
                Err(e) => {
                    log::warn!("Could not create blend buffer: {}", ErrorFmt(e));
                }
            }
        }
        let blend_buffer = blend_buffer.as_ref();
        let dev_ctx = &dev.ctx.get();
        let common = CommonArgs {
            dev,
            dev_ctx,
            format,
            plane_modifiers,
            width,
            height,
            render_ctx,
            cursor,
            blend_buffer,
        };
        if render_ctx.dev_id == dev.id {
            return wrap_error(&common, None, |dbg| {
                let prepared = &self.prepare_prime_none(&common, dbg)?;
                self.create_scanout_buffers_(|damage| {
                    self.create_scanout_buffer_prime_none(prepared, damage, dbg)
                })
            })
            .map_err(MetalError::AllocateScanoutBuffer);
        }
        let mut errors = ScanoutBufferErrors::default();
        for &method in &*PRIME_METHODS {
            let res = wrap_error(&common, Some(method), |dbg| {
                macro_rules! x {
                    ($prepare:ident, $create:ident $(,)?) => {{
                        let prepared = &self.$prepare(&common, dbg)?;
                        self.create_scanout_buffers_(|damage| self.$create(prepared, damage, dbg))
                    }};
                }
                match method {
                    PrimeMethod::DirectPull => x!(
                        prepare_direct_copy_pull,
                        create_scanout_buffer_prime_direct_copy_pull,
                    ),
                    PrimeMethod::DirectPush => x!(
                        prepare_direct_copy_push,
                        create_scanout_buffer_prime_direct_copy_push,
                    ),
                    PrimeMethod::Udmabuf => {
                        x!(prepare_copy_udmabuf, create_scanout_buffer_prime_udmabuf)
                    }
                    PrimeMethod::Sampling => {
                        x!(prepare_prime_sampling, create_scanout_buffer_prime_sampling)
                    }
                    PrimeMethod::IndirectPull => x!(
                        prepare_indirect_copy_pull,
                        create_scanout_buffer_prime_indirect_copy_pull,
                    ),
                }
            });
            match res {
                Err(e) => errors.errors.push(e),
                Ok(b) => {
                    if errors.errors.is_not_empty() {
                        let prefix = "Preferred prime methods failed";
                        if log::log_enabled!(Level::Debug) {
                            log::warn!("{prefix}: {}", ErrorFmt(errors));
                        } else {
                            let methods = fmt::from_fn(|f| {
                                for (idx, method) in
                                    errors.errors.iter().filter_map(|e| e.prime).enumerate()
                                {
                                    if idx > 0 {
                                        write!(f, ", ")?;
                                    }
                                    write!(f, "{method}")?;
                                }
                                Ok(())
                            });
                            log::warn!("{prefix}: {methods}");
                        }
                        log::warn!("Using {method}");
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

    fn prepare_prime_none<'a>(
        &self,
        common: &'a CommonArgs<'a>,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<NoPrime<'a>, ScanoutBufferErrorKind> {
        let dbg = &mut *dbg.borrow_mut();
        let CommonArgs {
            format,
            plane_modifiers,
            render_ctx,
            ..
        } = *common;
        let gfx_formats = render_ctx.gfx.formats();
        let Some(gfx_format) = gfx_formats.get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::RenderUnsupportedFormat);
        };
        dbg.render_gfx_write_modifiers = Some(gfx_format.write_modifiers.keys().copied().collect());
        dbg.render_gfx_read_modifiers = Some(gfx_format.read_modifiers.iter().copied().collect());
        let modifiers: IndexMap<_, _> = gfx_format
            .write_modifiers
            .iter()
            .map(|(m, v)| (*m, v))
            .filter(|(m, _)| plane_modifiers.contains(m))
            .filter(|(m, _)| gfx_format.read_modifiers.contains(m))
            .collect();
        dbg.render_modifiers_possible = Some(modifiers.keys().copied().collect());
        if modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::RenderNoWritableModifier);
        }
        let allocation_settings =
            BoAllocationSettings::new1(common, &modifiers, true, true, &mut dbg.render_usage);
        Ok(NoPrime {
            common,
            allocation_settings,
        })
    }

    fn create_scanout_buffer_prime_none(
        &self,
        prepared: &NoPrime<'_>,
        damage_queue: DamageQueue,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let NoPrime {
            common,
            allocation_settings,
        } = prepared;
        let CommonArgs {
            dev,
            format,
            width,
            height,
            render_ctx,
            blend_buffer,
            ..
        } = **common;
        let render = self.create_render_buffer_render(common, allocation_settings, dbg)?;
        let send_render_modifier = on_drop(|| {
            dbg.borrow_mut().render_modifier = Some(render.bo.dmabuf().modifier);
        });
        let drm = dev
            .master
            .add_fb(render.bo.dmabuf(), None)
            .map(Rc::new)
            .map_err(ScanoutBufferErrorKind::SodAddfb2)?;
        send_render_modifier.forget();
        Ok(RenderBuffer {
            width,
            height,
            locked: Cell::new(true),
            format,
            drm,
            damage_queue,
            blend_buffer: blend_buffer.cloned(),
            render,
            dev_ctx: render_ctx.clone(),
            prime: RenderBufferPrime::None,
        })
    }

    fn prepare_prime_sampling<'a>(
        &self,
        common: &'a CommonArgs<'a>,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<PrimeSampling<'a>, ScanoutBufferErrorKind> {
        let dbg = &mut *dbg.borrow_mut();
        let CommonArgs {
            dev_ctx,
            format,
            plane_modifiers,
            render_ctx,
            ..
        } = *common;
        let dev_gfx_formats = dev_ctx.gfx.formats();
        let Some(dev_gfx_format) = dev_gfx_formats.get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::SodUnsupportedFormat);
        };
        let render_gfx_formats = render_ctx.gfx.formats();
        let Some(render_gfx_format) = render_gfx_formats.get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::RenderUnsupportedFormat);
        };
        dbg.dev_gfx_read_modifiers = Some(dev_gfx_format.read_modifiers.iter().copied().collect());
        dbg.render_gfx_write_modifiers =
            Some(render_gfx_format.write_modifiers.keys().copied().collect());
        dbg.render_gfx_read_modifiers =
            Some(render_gfx_format.read_modifiers.iter().copied().collect());
        let render_modifiers_possible: IndexMap<_, _> = render_gfx_format
            .write_modifiers
            .iter()
            .filter(|(m, _)| render_gfx_format.read_modifiers.contains(*m))
            .filter(|(m, _)| dev_gfx_format.read_modifiers.contains(*m))
            .map(|(m, v)| (*m, v))
            .collect();
        dbg.render_modifiers_possible = Some(render_modifiers_possible.keys().copied().collect());
        if render_modifiers_possible.is_empty() {
            return Err(ScanoutBufferErrorKind::RenderNoWritableModifier);
        }
        dbg.dev_gfx_write_modifiers =
            Some(dev_gfx_format.write_modifiers.keys().copied().collect());
        let dev_modifiers_possible: IndexMap<_, _> = dev_gfx_format
            .write_modifiers
            .iter()
            .filter(|(m, _)| plane_modifiers.contains(*m))
            .map(|(m, v)| (*m, v))
            .collect();
        dbg.dev_modifiers_possible = Some(dev_modifiers_possible.keys().copied().collect());
        if dev_modifiers_possible.is_empty() {
            return Err(ScanoutBufferErrorKind::SodNoWritableModifier);
        }
        let render_allocation_settings = BoAllocationSettings::new1(
            common,
            &render_modifiers_possible,
            false,
            true,
            &mut dbg.render_usage,
        );
        let dev_allocation_settings = BoAllocationSettings::new1(
            common,
            &dev_modifiers_possible,
            true,
            true,
            &mut dbg.dev_usage,
        );
        Ok(PrimeSampling {
            common,
            render_allocation_settings,
            dev_allocation_settings,
        })
    }

    fn create_scanout_buffer_prime_sampling(
        &self,
        prepared: &PrimeSampling<'_>,
        damage_queue: DamageQueue,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let PrimeSampling {
            common,
            render_allocation_settings,
            dev_allocation_settings,
        } = prepared;
        let CommonArgs {
            dev,
            format,
            width,
            height,
            blend_buffer,
            ..
        } = **common;
        let dev_ctx = dev.ctx.get();
        let render = self.create_render_buffer_render(common, render_allocation_settings, dbg)?;
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
        let dev_bo = dev_ctx
            .gbm
            .create_bo(
                &self.state.dma_buf_ids,
                width,
                height,
                format,
                &dev_allocation_settings.modifiers,
                dev_allocation_settings.usage,
            )
            .map_err(ScanoutBufferErrorKind::SodBufferAllocation)?;
        let send_dev_modifier = on_drop(|| {
            dbg.borrow_mut().dev_modifier = Some(dev_bo.dmabuf().modifier);
        });
        let drm = dev
            .master
            .add_fb(dev_bo.dmabuf(), None)
            .map(Rc::new)
            .map_err(ScanoutBufferErrorKind::SodAddfb2)?;
        let dev_fb = dev_ctx
            .gfx
            .clone()
            .dmabuf_img(dev_bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::SodImportSodImage)?
            .to_framebuffer()
            .map_err(ScanoutBufferErrorKind::SodImportFb)?;
        send_dev_modifier.forget();
        send_render_modifier.forget();
        Ok(RenderBuffer {
            width,
            height,
            locked: Cell::new(true),
            format,
            drm,
            damage_queue,
            blend_buffer: blend_buffer.cloned(),
            render,
            dev_ctx,
            prime: RenderBufferPrime::Sampling {
                dev_bo,
                dev_fb,
                dev_render_tex,
            },
        })
    }

    fn prepare_direct_copy_push<'a>(
        &self,
        common: &'a CommonArgs<'a>,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<DirectCopyPush<'a>, ScanoutBufferErrorKind> {
        let dbg = &mut *dbg.borrow_mut();
        let CommonArgs {
            dev,
            format,
            plane_modifiers,
            render_ctx,
            ..
        } = *common;
        let Some(render_fmt) = render_ctx.gfx.formats().get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::RenderUnsupportedFormat);
        };
        let Some(render_copy_dev) = render_ctx.copy_device.get() else {
            return Err(ScanoutBufferErrorKind::RenderNoCopyDevice);
        };
        let Some(dev_copy_dev) = dev.copy_device.get() else {
            return Err(ScanoutBufferErrorKind::SodNoCopyDevice);
        };
        let render_copy_src_modifiers: Vec<_> =
            copy_modifiers(common, render_copy_dev.physical().src_support()).collect();
        let render_gfx_write_modifiers: Vec<_> =
            render_fmt.write_modifiers.keys().copied().collect();
        let render_modifiers = intersect_modifiers(
            render_gfx_write_modifiers
                .iter()
                .copied()
                .filter(|m| render_fmt.read_modifiers.contains(m)),
            render_copy_src_modifiers.iter().copied(),
        );
        dbg.render_gfx_write_modifiers = Some(render_gfx_write_modifiers);
        dbg.render_gfx_read_modifiers = Some(render_fmt.read_modifiers.iter().copied().collect());
        dbg.render_copy_src_modifiers = Some(render_copy_src_modifiers);
        dbg.render_modifiers_possible = Some(render_modifiers.clone());
        if render_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::RenderNoWritableModifier);
        }
        let render_copy_dst_modifiers: Vec<_> =
            copy_modifiers(common, render_copy_dev.physical().dst_support()).collect();
        let mut dev_modifiers = intersect_modifiers(
            plane_modifiers.iter().copied(),
            render_copy_dst_modifiers.iter().copied(),
        );
        dbg.render_copy_dst_modifiers = Some(render_copy_dst_modifiers);
        make_linear_only(&mut dev_modifiers);
        dbg.dev_modifiers_possible = Some(dev_modifiers.clone());
        if dev_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::SodNoWritableModifier);
        }
        let render_allocation_settings = BoAllocationSettings::new2(
            common,
            &render_modifiers,
            render_fmt,
            false,
            true,
            &mut dbg.render_usage,
        );
        let dev_allocation_settings =
            BoAllocationSettings::new3(common, &dev_modifiers, true, false, &mut dbg.dev_usage);
        Ok(DirectCopyPush {
            common,
            render_copy_dev,
            dev_copy_dev,
            render_allocation_settings,
            dev_allocation_settings,
        })
    }

    fn create_scanout_buffer_prime_direct_copy_push(
        &self,
        prepared: &DirectCopyPush<'_>,
        damage_queue: DamageQueue,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let DirectCopyPush {
            common,
            render_copy_dev,
            dev_copy_dev,
            render_allocation_settings,
            dev_allocation_settings,
        } = prepared;
        let CommonArgs {
            dev,
            format,
            width,
            height,
            blend_buffer,
            ..
        } = **common;
        let dev_ctx = dev.ctx.get();
        let render = self.create_render_buffer_render(common, render_allocation_settings, dbg)?;
        let send_render_modifier = on_drop(|| {
            dbg.borrow_mut().render_modifier = Some(render.bo.dmabuf().modifier);
        });
        let dev_bo = dev_ctx
            .gbm
            .create_bo(
                &self.state.dma_buf_ids,
                width,
                height,
                format,
                &dev_allocation_settings.modifiers,
                dev_allocation_settings.usage,
            )
            .map_err(ScanoutBufferErrorKind::SodBufferAllocation)?;
        let send_dev_modifier = on_drop(|| {
            dbg.borrow_mut().dev_modifier = Some(dev_bo.dmabuf().modifier);
        });
        let render_copy = render_copy_dev
            .create_copy(&render.bo.dmabuf(), &dev_bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::RenderCreateCopyToSod)?;
        let drm = dev
            .master
            .add_fb(dev_bo.dmabuf(), None)
            .map(Rc::new)
            .map_err(ScanoutBufferErrorKind::SodAddfb2)?;
        send_dev_modifier.forget();
        send_render_modifier.forget();
        Ok(RenderBuffer {
            width,
            height,
            locked: Cell::new(true),
            format,
            drm,
            damage_queue,
            blend_buffer: blend_buffer.cloned(),
            render,
            dev_ctx,
            prime: RenderBufferPrime::CopyDirectPush {
                dev_copy_dev: dev_copy_dev.clone(),
                render_copy,
                dev_bo,
            },
        })
    }

    fn prepare_direct_copy_pull<'a>(
        &self,
        common: &'a CommonArgs<'a>,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<DirectCopyPull<'a>, ScanoutBufferErrorKind> {
        let dbg = &mut *dbg.borrow_mut();
        let CommonArgs {
            dev,
            format,
            plane_modifiers,
            render_ctx,
            ..
        } = *common;
        let Some(render_fmt) = render_ctx.gfx.formats().get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::RenderUnsupportedFormat);
        };
        let Some(dev_copy_dev) = dev.copy_device.get() else {
            return Err(ScanoutBufferErrorKind::SodNoCopyDevice);
        };
        let dev_copy_src_modifiers: Vec<_> =
            copy_modifiers(common, dev_copy_dev.physical().src_support()).collect();
        let render_gfx_write_modifiers: Vec<_> =
            render_fmt.write_modifiers.keys().copied().collect();
        let render_modifiers = intersect_modifiers(
            render_gfx_write_modifiers
                .iter()
                .copied()
                .filter(|m| render_fmt.read_modifiers.contains(m)),
            dev_copy_src_modifiers.iter().copied(),
        );
        dbg.render_gfx_write_modifiers = Some(render_gfx_write_modifiers);
        dbg.render_gfx_read_modifiers = Some(render_fmt.read_modifiers.iter().copied().collect());
        dbg.dev_copy_src_modifiers = Some(dev_copy_src_modifiers);
        dbg.render_modifiers_possible = Some(render_modifiers.clone());
        if render_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::RenderNoWritableModifier);
        }
        let dev_copy_dst_modifiers: Vec<_> =
            copy_modifiers(common, dev_copy_dev.physical().dst_support()).collect();
        let mut dev_modifiers = intersect_modifiers(
            plane_modifiers.iter().copied(),
            dev_copy_dst_modifiers.iter().copied(),
        );
        dbg.dev_copy_dst_modifiers = Some(dev_copy_dst_modifiers);
        make_linear_only(&mut dev_modifiers);
        dbg.dev_modifiers_possible = Some(dev_modifiers.clone());
        if dev_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::SodNoWritableModifier);
        }
        let render_allocation_settings = BoAllocationSettings::new2(
            common,
            &render_modifiers,
            render_fmt,
            false,
            true,
            &mut dbg.render_usage,
        );
        let dev_allocation_settings =
            BoAllocationSettings::new3(common, &dev_modifiers, true, false, &mut dbg.dev_usage);
        Ok(DirectCopyPull {
            common,
            dev_copy_dev,
            render_allocation_settings,
            dev_allocation_settings,
        })
    }

    fn create_scanout_buffer_prime_direct_copy_pull(
        &self,
        prepared: &DirectCopyPull<'_>,
        damage_queue: DamageQueue,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let DirectCopyPull {
            common,
            dev_copy_dev,
            render_allocation_settings,
            dev_allocation_settings,
        } = prepared;
        let CommonArgs {
            dev,
            format,
            width,
            height,
            blend_buffer,
            ..
        } = **common;
        let dev_ctx = dev.ctx.get();
        let render = self.create_render_buffer_render(common, render_allocation_settings, dbg)?;
        let send_render_modifier = on_drop(|| {
            dbg.borrow_mut().render_modifier = Some(render.bo.dmabuf().modifier);
        });
        let dev_bo = dev_ctx
            .gbm
            .create_bo(
                &self.state.dma_buf_ids,
                width,
                height,
                format,
                &dev_allocation_settings.modifiers,
                dev_allocation_settings.usage,
            )
            .map_err(ScanoutBufferErrorKind::SodBufferAllocation)?;
        let send_dev_modifier = on_drop(|| {
            dbg.borrow_mut().dev_modifier = Some(dev_bo.dmabuf().modifier);
        });
        let dev_copy = dev_copy_dev
            .create_copy(&render.bo.dmabuf(), &dev_bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::SodCreateCopyFromRender)?;
        let drm = dev
            .master
            .add_fb(dev_bo.dmabuf(), None)
            .map(Rc::new)
            .map_err(ScanoutBufferErrorKind::SodAddfb2)?;
        send_dev_modifier.forget();
        send_render_modifier.forget();
        Ok(RenderBuffer {
            width,
            height,
            locked: Cell::new(true),
            format,
            drm,
            damage_queue,
            blend_buffer: blend_buffer.cloned(),
            render,
            dev_ctx,
            prime: RenderBufferPrime::CopyDirectPull {
                dev_copy_dev: dev_copy_dev.clone(),
                dev_copy,
                dev_bo,
            },
        })
    }

    fn prepare_indirect_copy_pull<'a>(
        &self,
        common: &'a CommonArgs<'a>,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<IndirectCopyPull<'a>, ScanoutBufferErrorKind> {
        let dbg = &mut *dbg.borrow_mut();
        let CommonArgs {
            dev,
            format,
            plane_modifiers,
            render_ctx,
            ..
        } = *common;
        let Some(render_fmt) = render_ctx.gfx.formats().get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::RenderUnsupportedFormat);
        };
        let Some(render_copy_dev) = render_ctx.copy_device.get() else {
            return Err(ScanoutBufferErrorKind::RenderNoCopyDevice);
        };
        let Some(dev_copy_dev) = dev.copy_device.get() else {
            return Err(ScanoutBufferErrorKind::SodNoCopyDevice);
        };
        let render_copy_src_modifiers: Vec<_> =
            copy_modifiers(common, render_copy_dev.physical().src_support()).collect();
        let render_gfx_write_modifiers: Vec<_> =
            render_fmt.write_modifiers.keys().copied().collect();
        let render_modifiers = intersect_modifiers(
            render_gfx_write_modifiers
                .iter()
                .copied()
                .filter(|m| render_fmt.read_modifiers.contains(m)),
            render_copy_src_modifiers.iter().copied(),
        );
        dbg.render_copy_src_modifiers = Some(render_copy_src_modifiers);
        dbg.render_gfx_read_modifiers = Some(render_fmt.read_modifiers.iter().copied().collect());
        dbg.render_gfx_write_modifiers = Some(render_gfx_write_modifiers);
        dbg.render_modifiers_possible = Some(render_modifiers.clone());
        if render_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::RenderNoWritableModifier);
        }
        if !copy_supports_linear(common, render_copy_dev.physical().dst_support()) {
            return Err(ScanoutBufferErrorKind::RenderNoCopyToLinear);
        }
        if !copy_supports_linear(common, dev_copy_dev.physical().src_support()) {
            return Err(ScanoutBufferErrorKind::SodNoCopyFromLinear);
        }
        let dev_copy_dst_modifiers: Vec<_> =
            copy_modifiers(common, dev_copy_dev.physical().dst_support()).collect();
        let mut dev_modifiers = intersect_modifiers(
            plane_modifiers.iter().copied(),
            dev_copy_dst_modifiers.iter().copied(),
        );
        dbg.dev_copy_dst_modifiers = Some(dev_copy_dst_modifiers);
        make_linear_only(&mut dev_modifiers);
        dbg.dev_modifiers_possible = Some(dev_modifiers.clone());
        if dev_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::SodNoWritableModifier);
        }
        let render_allocation_settings = BoAllocationSettings::new2(
            common,
            &render_modifiers,
            render_fmt,
            false,
            true,
            &mut dbg.render_usage,
        );
        let dev_allocation_settings =
            BoAllocationSettings::new3(common, &dev_modifiers, true, false, &mut dbg.dev_usage);
        Ok(IndirectCopyPull {
            common,
            render_allocation_settings,
            render_copy_dev,
            dev_copy_dev,
            dev_allocation_settings,
        })
    }

    fn create_scanout_buffer_prime_indirect_copy_pull(
        &self,
        prepared: &IndirectCopyPull,
        damage_queue: DamageQueue,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let IndirectCopyPull {
            common,
            render_allocation_settings,
            render_copy_dev,
            dev_copy_dev,
            dev_allocation_settings,
        } = prepared;
        let CommonArgs {
            dev,
            format,
            width,
            height,
            blend_buffer,
            ..
        } = **common;
        let render_secondary_bo = render_copy_dev
            .create_buffer(&self.state.dma_buf_ids, width, height, format)
            .map_err(ScanoutBufferErrorKind::RenderCreateCopyBuffer)?;
        let dev_ctx = dev.ctx.get();
        let render = self.create_render_buffer_render(common, render_allocation_settings, dbg)?;
        let send_render_modifier = on_drop(|| {
            dbg.borrow_mut().render_modifier = Some(render.bo.dmabuf().modifier);
        });
        let dev_bo = dev_ctx
            .gbm
            .create_bo(
                &self.state.dma_buf_ids,
                width,
                height,
                format,
                &dev_allocation_settings.modifiers,
                dev_allocation_settings.usage,
            )
            .map_err(ScanoutBufferErrorKind::SodBufferAllocation)?;
        let send_dev_modifier = on_drop(|| {
            dbg.borrow_mut().dev_modifier = Some(dev_bo.dmabuf().modifier);
        });
        let render_copy = render_copy_dev
            .create_copy(render.bo.dmabuf(), render_secondary_bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::RenderCreateCopyToUdmabuf)?;
        let dev_copy = dev_copy_dev
            .create_copy(render_secondary_bo.dmabuf(), dev_bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::SodCreateCopyFromUdmabuf)?;
        let drm = dev
            .master
            .add_fb(dev_bo.dmabuf(), None)
            .map(Rc::new)
            .map_err(ScanoutBufferErrorKind::SodAddfb2)?;
        send_render_modifier.forget();
        send_dev_modifier.forget();
        Ok(RenderBuffer {
            width,
            height,
            locked: Cell::new(true),
            format,
            drm,
            damage_queue,
            blend_buffer: blend_buffer.cloned(),
            render,
            dev_ctx,
            prime: RenderBufferPrime::CopyIndirectPull {
                render_copy,
                _render_secondary_bo: render_secondary_bo,
                dev_copy_dev: dev_copy_dev.clone(),
                dev_copy,
                dev_bo,
            },
        })
    }

    fn prepare_copy_udmabuf<'a>(
        &self,
        common: &'a CommonArgs<'a>,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<CopyUdmabuf<'a>, ScanoutBufferErrorKind> {
        let dbg = &mut *dbg.borrow_mut();
        let CommonArgs {
            dev,
            format,
            plane_modifiers,
            render_ctx,
            ..
        } = *common;
        let Some(udmabuf) = self.state.udmabuf.get() else {
            return Err(ScanoutBufferErrorKind::UdmabufNotAvailable);
        };
        let Some(render_fmt) = render_ctx.gfx.formats().get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::RenderUnsupportedFormat);
        };
        let Some(render_copy_dev) = render_ctx.copy_device.get() else {
            return Err(ScanoutBufferErrorKind::RenderNoCopyDevice);
        };
        if !copy_supports_linear(common, render_copy_dev.physical().dst_support()) {
            return Err(ScanoutBufferErrorKind::RenderNoCopyToLinear);
        }
        let Some(dev_copy_dev) = dev.copy_device.get() else {
            return Err(ScanoutBufferErrorKind::SodNoCopyDevice);
        };
        if !copy_supports_linear(common, dev_copy_dev.physical().src_support()) {
            return Err(ScanoutBufferErrorKind::SodNoCopyFromLinear);
        }
        let render_copy_src_modifiers: Vec<_> =
            copy_modifiers(common, render_copy_dev.physical().src_support()).collect();
        let render_gfx_write_modifiers: Vec<_> =
            render_fmt.write_modifiers.keys().copied().collect();
        let render_modifiers = intersect_modifiers(
            render_gfx_write_modifiers
                .iter()
                .copied()
                .filter(|m| render_fmt.read_modifiers.contains(m)),
            render_copy_src_modifiers.iter().copied(),
        );
        dbg.render_copy_src_modifiers = Some(render_copy_src_modifiers);
        dbg.render_gfx_read_modifiers = Some(render_fmt.read_modifiers.iter().copied().collect());
        dbg.render_gfx_write_modifiers = Some(render_gfx_write_modifiers);
        dbg.render_modifiers_possible = Some(render_modifiers.clone());
        if render_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::RenderNoWritableModifier);
        }
        let dev_copy_dst_modifiers: Vec<_> =
            copy_modifiers(common, dev_copy_dev.physical().dst_support()).collect();
        let mut dev_modifiers = intersect_modifiers(
            plane_modifiers.iter().copied(),
            dev_copy_dst_modifiers.iter().copied(),
        );
        dbg.dev_copy_dst_modifiers = Some(dev_copy_dst_modifiers);
        make_linear_only(&mut dev_modifiers);
        dbg.dev_modifiers_possible = Some(dev_modifiers.clone());
        if dev_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::SodNoWritableModifier);
        }
        let render_allocation_settings = BoAllocationSettings::new2(
            common,
            &render_modifiers,
            render_fmt,
            false,
            true,
            &mut dbg.render_usage,
        );
        let dev_allocation_settings =
            BoAllocationSettings::new3(common, &dev_modifiers, true, false, &mut dbg.dev_usage);
        Ok(CopyUdmabuf {
            common,
            udmabuf,
            render_allocation_settings,
            render_copy_dev,
            dev_copy_dev,
            dev_allocation_settings,
        })
    }

    fn create_scanout_buffer_prime_udmabuf(
        &self,
        prepared: &CopyUdmabuf<'_>,
        damage_queue: DamageQueue,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let CopyUdmabuf {
            common,
            udmabuf,
            render_allocation_settings,
            render_copy_dev,
            dev_copy_dev,
            dev_allocation_settings,
        } = prepared;
        let CommonArgs {
            dev,
            format,
            width,
            height,
            blend_buffer,
            ..
        } = **common;
        let udmabuf = udmabuf
            .create_dmabuf(&self.state.dma_buf_ids, width, height, format)
            .map_err(ScanoutBufferErrorKind::CreateUdmabuf)?;
        let dev_ctx = dev.ctx.get();
        let render = self.create_render_buffer_render(common, render_allocation_settings, dbg)?;
        let send_render_modifier = on_drop(|| {
            dbg.borrow_mut().render_modifier = Some(render.bo.dmabuf().modifier);
        });
        let dev_bo = dev_ctx
            .gbm
            .create_bo(
                &self.state.dma_buf_ids,
                width,
                height,
                format,
                &dev_allocation_settings.modifiers,
                dev_allocation_settings.usage,
            )
            .map_err(ScanoutBufferErrorKind::SodBufferAllocation)?;
        let send_dev_modifier = on_drop(|| {
            dbg.borrow_mut().dev_modifier = Some(dev_bo.dmabuf().modifier);
        });
        let render_copy = render_copy_dev
            .create_copy(&render.bo.dmabuf(), &udmabuf)
            .map_err(ScanoutBufferErrorKind::RenderCreateCopyToUdmabuf)?;
        let dev_copy = dev_copy_dev
            .create_copy(&udmabuf, &dev_bo.dmabuf())
            .map_err(ScanoutBufferErrorKind::SodCreateCopyFromUdmabuf)?;
        let drm = dev
            .master
            .add_fb(dev_bo.dmabuf(), None)
            .map(Rc::new)
            .map_err(ScanoutBufferErrorKind::SodAddfb2)?;
        send_render_modifier.forget();
        send_dev_modifier.forget();
        Ok(RenderBuffer {
            width,
            height,
            locked: Cell::new(true),
            format,
            drm,
            damage_queue,
            blend_buffer: blend_buffer.cloned(),
            render,
            dev_ctx,
            prime: RenderBufferPrime::CopyUdmabuf {
                render_copy,
                dev_copy_dev: dev_copy_dev.clone(),
                dev_copy,
                dev_bo,
            },
        })
    }

    fn create_render_buffer_render(
        &self,
        common: &CommonArgs<'_>,
        settings: &BoAllocationSettings,
        dbg: &RefCell<RenderBufferAllocationDebug>,
    ) -> Result<RenderBufferRender, ScanoutBufferErrorKind> {
        let CommonArgs {
            format,
            width,
            height,
            render_ctx,
            ..
        } = *common;
        let bo = render_ctx
            .gbm
            .create_bo(
                &self.state.dma_buf_ids,
                width,
                height,
                format,
                &settings.modifiers,
                settings.usage,
            )
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
            .clone()
            .to_framebuffer()
            .map_err(ScanoutBufferErrorKind::RenderImportFb)?;
        fb.clear(
            AcquireSync::Unnecessary,
            ReleaseSync::None,
            self.state.color_manager.srgb_gamma22(),
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
}

#[derive(Debug, Error)]
pub enum ScanoutBufferErrorKind {
    #[error("Scanout device: The format is not supported")]
    SodUnsupportedFormat,
    #[error(
        "Scanout device: The intersection of the modifiers supported by the plane and modifiers writable by the gfx API is empty"
    )]
    SodNoWritableModifier,
    #[error("Scanout device: Buffer allocation failed")]
    SodBufferAllocation(#[source] GbmError),
    #[error("Scanout device: addfb2 failed")]
    SodAddfb2(#[source] DrmError),
    #[error("Scanout device: Could not import SCANOUT buffer into the gfx API")]
    SodImportSodImage(#[source] GfxError),
    #[error("Scanout device: Could not turn imported SCANOUT buffer into gfx API FB")]
    SodImportFb(#[source] GfxError),
    #[error("Render device: The format is not supported")]
    RenderUnsupportedFormat,
    #[error(
        "Render device: The intersection of the modifiers readable by the scanout device and modifiers writable by the gfx API is empty"
    )]
    RenderNoWritableModifier,
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
    #[error("Udmabuf is not available")]
    UdmabufNotAvailable,
    #[error("Render device: Could not create a copy device")]
    RenderNoCopyDevice,
    #[error("Scanout device: Could not create a copy device")]
    SodNoCopyDevice,
    #[error("Render device: Cannot copy to linear")]
    RenderNoCopyToLinear,
    #[error("Scanout device: Cannot copy from linear")]
    SodNoCopyFromLinear,
    #[error("Could not create an udmabuf")]
    CreateUdmabuf(#[source] UdmabufError),
    #[error("Render device: Could not create a copy to udmabuf")]
    RenderCreateCopyToUdmabuf(#[source] CopyDeviceError),
    #[error("Scanout device: Could not create a copy from udmabuf")]
    SodCreateCopyFromUdmabuf(#[source] CopyDeviceError),
    #[error("Scanout device: Could not create a copy from render bo")]
    SodCreateCopyFromRender(#[source] CopyDeviceError),
    #[error("Render device: Could not create a copy to scanout device")]
    RenderCreateCopyToSod(#[source] CopyDeviceError),
    #[error("Render device: Copy buffer allocation failed")]
    RenderCreateCopyBuffer(#[source] CopyDeviceError),
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
    DirectPull,
    Sampling,
    IndirectPull,
    Udmabuf,
    // This does not work on AMD since use from another device will prevent the
    // framebuffer from being pinned into video memory. It might be useful on other
    // devices where the scanout device is CPU only and the render device can perform
    // an accelerated copy.
    DirectPush,
}

impl PrimeMethod {
    pub fn name(self) -> &'static str {
        match self {
            PrimeMethod::DirectPull => "direct-pull",
            PrimeMethod::IndirectPull => "indirect-pull",
            PrimeMethod::DirectPush => "direct-push",
            PrimeMethod::Sampling => "cross-device-sampling",
            PrimeMethod::Udmabuf => "udmabuf",
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
            RenderBufferPrime::CopyUdmabuf { .. } => PrimeMethod::Udmabuf,
            RenderBufferPrime::CopyDirectPull { .. } => PrimeMethod::DirectPull,
            RenderBufferPrime::CopyDirectPush { .. } => PrimeMethod::DirectPush,
            RenderBufferPrime::CopyIndirectPull { .. } => PrimeMethod::IndirectPull,
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
    common: &CommonArgs<'_>,
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
        common: &CommonArgs<'_>,
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

    fn new2<'a>(
        common: &CommonArgs<'_>,
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
        common: &CommonArgs<'_>,
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

const JAY_PRIME_METHODS: &str = "JAY_PRIME_METHODS";

static PRIME_METHODS: LazyLock<ArrayVec<PrimeMethod, { PrimeMethod::LENGTH }>> =
    LazyLock::new(prime_methods);

fn prime_methods() -> ArrayVec<PrimeMethod, { PrimeMethod::LENGTH }> {
    let mut res = ArrayVec::new();
    let mut used = StaticMap::<_, bool>::default();
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
            if !used[method] {
                used[method] = true;
                if allow {
                    res.push(method);
                }
            }
        }
    }
    for method in PrimeMethod::variants() {
        if !used[method] {
            used[method] = true;
            res.push(method);
        }
    }
    log::info!("Prime methods: {:?}", res);
    res
}

fn intersect_modifiers(
    left: impl IntoIterator<Item = Modifier>,
    right: impl IntoIterator<Item = Modifier>,
) -> Vec<Modifier> {
    let right: HashSet<_> = right.into_iter().collect();
    left.into_iter().filter(|m| right.contains(m)).collect()
}

fn copy_modifiers(
    common: &CommonArgs<'_>,
    support: &[CopyDeviceSupport],
) -> impl Iterator<Item = Modifier> {
    let CommonArgs {
        format,
        width,
        height,
        ..
    } = *common;
    support
        .iter()
        .filter(move |s| {
            s.format == format && s.max_width >= width as _ && s.max_height >= height as _
        })
        .map(move |s| s.modifier)
}

fn copy_supports_linear(common: &CommonArgs<'_>, support: &[CopyDeviceSupport]) -> bool {
    copy_modifiers(common, support).any(|m| m == LINEAR_MODIFIER)
}

fn make_linear_only(modifiers: &mut Vec<Modifier>) {
    if modifiers.contains(&LINEAR_MODIFIER) {
        *modifiers = vec![LINEAR_MODIFIER];
    }
}
