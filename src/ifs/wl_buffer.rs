use {
    crate::{
        client::{Client, ClientError},
        clientmem::{ClientMem, ClientMemError, ClientMemOffset},
        format::{ARGB8888, Format},
        gfx_api::{GfxBuffer, GfxContext, GfxError, GfxFramebuffer, GfxImage, GfxTexture},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        rect::{Rect, Region},
        utils::{errorfmt::ErrorFmt, event_listener::EventListener, page_size::page_size},
        video::{
            LINEAR_MODIFIER,
            dmabuf::{DmaBuf, DmaBufPlane},
        },
        wire::{WlBufferId, wl_buffer::*},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
    uapi::OwnedFd,
};

pub enum WlBufferStorage {
    Shm {
        mem: Rc<ClientMemOffset>,
        stride: i32,
        dmabuf_buffer_params: DmabufBufferParams,
    },
    Dmabuf {
        img: Rc<dyn GfxImage>,
        tex: Option<Rc<dyn GfxTexture>>,
        fb: Option<Rc<dyn GfxFramebuffer>>,
    },
}

pub struct DmabufBufferParams {
    size: usize,
    udmabuf: Option<Rc<OwnedFd>>,
    udmabuf_offset: usize,
    udmabuf_size: usize,
    udmabuf_impossible: bool,
    host_buffer: Option<Rc<dyn GfxBuffer>>,
    host_buffer_impossible: bool,
    tex: Option<Rc<dyn GfxTexture>>,
    tex_impossible: bool,
}

pub struct WlBuffer {
    pub id: WlBufferId,
    destroyed: Cell<bool>,
    pub client: Rc<Client>,
    pub rect: Rect,
    pub format: &'static Format,
    pub dmabuf: Option<DmaBuf>,
    render_ctx_version: Cell<u32>,
    pub storage: RefCell<Option<WlBufferStorage>>,
    shm: bool,
    pub color: Option<[u32; 4]>,
    width: i32,
    height: i32,
    gfx_ctx_changed: EventListener<WlBuffer>,
    pub tracker: Tracker<Self>,
}

impl WlBuffer {
    pub fn destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub fn is_shm(&self) -> bool {
        self.shm
    }

    fn new(
        id: WlBufferId,
        client: &Rc<Client>,
        format: &'static Format,
        width: i32,
        height: i32,
        dmabuf: Option<DmaBuf>,
        storage: Option<WlBufferStorage>,
        shm: bool,
        color: Option<[u32; 4]>,
    ) -> Rc<Self> {
        let slf = Rc::new_cyclic(|slf| Self {
            id,
            destroyed: Cell::new(false),
            client: client.clone(),
            rect: Rect::new_sized_saturating(0, 0, width, height),
            format,
            width,
            height,
            dmabuf,
            render_ctx_version: Cell::new(client.state.render_ctx_version.get()),
            storage: RefCell::new(storage),
            shm,
            tracker: Default::default(),
            color,
            gfx_ctx_changed: EventListener::new(slf.clone()),
        });
        slf.gfx_ctx_changed.attach(&client.gfx_ctx_changed);
        slf
    }

    pub fn new_dmabuf(
        id: WlBufferId,
        client: &Rc<Client>,
        format: &'static Format,
        dmabuf: DmaBuf,
        img: &Rc<dyn GfxImage>,
    ) -> Rc<Self> {
        Self::new(
            id,
            client,
            format,
            img.width(),
            img.height(),
            Some(dmabuf),
            Some(WlBufferStorage::Dmabuf {
                img: img.clone(),
                tex: None,
                fb: None,
            }),
            false,
            None,
        )
    }

    #[expect(clippy::too_many_arguments)]
    pub fn new_shm(
        id: WlBufferId,
        client: &Rc<Client>,
        offset: usize,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        mem: &Rc<ClientMem>,
        udmabuf: Option<(&Rc<OwnedFd>, usize)>,
    ) -> Result<Rc<Self>, WlBufferError> {
        let bytes = stride as u64 * height as u64;
        let required = bytes + offset as u64;
        if required > mem.len() as u64 {
            return Err(WlBufferError::OutOfBounds);
        }
        let mem = Rc::new(mem.offset(offset));
        let min_row_size = width as u64 * format.bpp as u64;
        if (stride as u64) < min_row_size {
            return Err(WlBufferError::StrideTooSmall);
        }
        let udmabuf_impossible = !mem.pool().is_sealed_memfd();
        let dmabuf_buffer_params = match udmabuf {
            None => DmabufBufferParams {
                size: bytes as usize,
                udmabuf: None,
                udmabuf_offset: 0,
                udmabuf_size: 0,
                udmabuf_impossible,
                host_buffer: None,
                host_buffer_impossible: udmabuf_impossible,
                tex: None,
                tex_impossible: udmabuf_impossible,
            },
            Some((udmabuf, size)) => DmabufBufferParams {
                size,
                udmabuf: Some(udmabuf.clone()),
                udmabuf_offset: offset,
                udmabuf_size: size,
                udmabuf_impossible: false,
                host_buffer: None,
                host_buffer_impossible: false,
                tex: None,
                tex_impossible: false,
            },
        };
        Ok(Self::new(
            id,
            client,
            format,
            width,
            height,
            None,
            Some(WlBufferStorage::Shm {
                dmabuf_buffer_params,
                mem,
                stride,
            }),
            true,
            None,
        ))
    }

    pub fn new_single_pixel(
        id: WlBufferId,
        client: &Rc<Client>,
        r: u32,
        g: u32,
        b: u32,
        a: u32,
    ) -> Rc<Self> {
        Self::new(
            id,
            client,
            ARGB8888,
            1,
            1,
            None,
            None,
            false,
            Some([r, g, b, a]),
        )
    }

    pub fn handle_gfx_context_change(&self) -> bool {
        let ctx_version = self.client.state.render_ctx_version.get();
        let up_to_date = self.render_ctx_version.replace(ctx_version) == ctx_version;
        let mut storage = self.storage.borrow_mut();
        let Some(s) = &mut *storage else {
            return false;
        };
        if up_to_date {
            let tex = match s {
                WlBufferStorage::Shm {
                    dmabuf_buffer_params: DmabufBufferParams { tex, .. },
                    ..
                } => tex,
                WlBufferStorage::Dmabuf { tex, .. } => tex,
            };
            return tex.is_some();
        }
        let had_texture = match s {
            WlBufferStorage::Shm {
                dmabuf_buffer_params:
                    DmabufBufferParams {
                        udmabuf_impossible,
                        host_buffer,
                        host_buffer_impossible,
                        tex,
                        tex_impossible,
                        ..
                    },
                ..
            } => {
                host_buffer.take();
                *host_buffer_impossible = *udmabuf_impossible;
                let had_texture = tex.take().is_some();
                *tex_impossible = *udmabuf_impossible;
                return had_texture;
            }
            WlBufferStorage::Dmabuf { tex, .. } => tex.is_some(),
        };
        *storage = None;
        let Some(ctx) = self.client.state.render_ctx.get() else {
            return false;
        };
        let Some(dmabuf) = &self.dmabuf else {
            return false;
        };
        let img = match ctx.dmabuf_img(dmabuf) {
            Ok(image) => image,
            Err(e) => {
                log::error!(
                    "Cannot re-import wl_buffer after graphics context change: {}",
                    ErrorFmt(e)
                );
                return false;
            }
        };
        *storage = Some(WlBufferStorage::Dmabuf {
            img,
            tex: None,
            fb: None,
        });
        had_texture
    }

    pub fn get_texture(&self, surface: &WlSurface) -> Option<Rc<dyn GfxTexture>> {
        match &*self.storage.borrow() {
            None => None,
            Some(s) => match s {
                WlBufferStorage::Shm {
                    dmabuf_buffer_params,
                    ..
                } => {
                    if let Some(tex) = &dmabuf_buffer_params.tex {
                        return Some(tex.clone());
                    }
                    surface.shm_textures.front().tex.get().map(|t| t as _)
                }
                WlBufferStorage::Dmabuf { tex, .. } => tex.clone(),
            },
        }
    }

    pub fn update_texture_or_log(&self, surface: &WlSurface, sync_shm: bool) {
        if let Err(e) = self.update_texture(surface, sync_shm) {
            log::warn!("Could not update texture: {}", ErrorFmt(e));
        }
    }

    pub fn get_udmabuf(
        &self,
        mem: &Rc<ClientMemOffset>,
        dmabuf_buffer_params: &mut DmabufBufferParams,
    ) -> Option<Rc<OwnedFd>> {
        let DmabufBufferParams {
            size,
            udmabuf,
            udmabuf_offset,
            udmabuf_size,
            udmabuf_impossible,
            ..
        } = dmabuf_buffer_params;
        if let Some(b) = udmabuf {
            return Some(b.clone());
        }
        if *udmabuf_impossible {
            return None;
        }
        let dev = self.client.state.udmabuf.get()?;
        let mask = page_size() - 1;
        let offset = mem.offset() & mask;
        let base = mem.offset() & !mask;
        let end = (mem.offset() + *size + mask) & !mask;
        let len = end - base;
        match dev.create_dmabuf_from_memfd(mem.pool().fd(), base, len) {
            Ok(b) => {
                let b = Rc::new(b);
                *udmabuf_offset = offset;
                *udmabuf_size = len;
                *udmabuf = Some(b.clone());
                Some(b)
            }
            Err(e) => {
                *udmabuf_impossible = true;
                log::debug!("Could not create udmabuf: {}", ErrorFmt(e));
                None
            }
        }
    }

    pub fn get_gfx_buffer(
        &self,
        ctx: &Rc<dyn GfxContext>,
        mem: &Rc<ClientMemOffset>,
        dmabuf_buffer_params: &mut DmabufBufferParams,
    ) -> Option<Rc<dyn GfxBuffer>> {
        let DmabufBufferParams {
            host_buffer,
            host_buffer_impossible,
            ..
        } = dmabuf_buffer_params;
        if let Some(hb) = host_buffer {
            return Some(hb.clone());
        }
        if *host_buffer_impossible {
            return None;
        }
        let udmabuf = self.get_udmabuf(mem, dmabuf_buffer_params)?;
        let DmabufBufferParams {
            udmabuf_offset,
            udmabuf_size,
            host_buffer,
            host_buffer_impossible,
            ..
        } = dmabuf_buffer_params;
        let hb =
            match ctx.create_dmabuf_buffer(&udmabuf, *udmabuf_offset, *udmabuf_size, self.format) {
                Ok(hb) => hb,
                Err(e) => {
                    *host_buffer_impossible = true;
                    log::debug!("Could not create gfx host buffer: {}", ErrorFmt(e));
                    return None;
                }
            };
        *host_buffer = Some(hb.clone());
        Some(hb)
    }

    pub fn import_udmabuf_texture(
        &self,
        ctx: &Rc<dyn GfxContext>,
        mem: &Rc<ClientMemOffset>,
        stride: i32,
        dmabuf_buffer_params: &mut DmabufBufferParams,
    ) -> bool {
        let DmabufBufferParams {
            tex,
            tex_impossible,
            ..
        } = dmabuf_buffer_params;
        if tex.is_some() {
            return true;
        }
        if *tex_impossible {
            return false;
        }
        let Some(udmabuf) = self.get_udmabuf(mem, dmabuf_buffer_params) else {
            return false;
        };
        let DmabufBufferParams {
            udmabuf_offset,
            tex,
            tex_impossible,
            ..
        } = dmabuf_buffer_params;
        let mut dmabuf = DmaBuf {
            id: self.client.state.dma_buf_ids.next(),
            width: self.width,
            height: self.height,
            format: self.format,
            modifier: LINEAR_MODIFIER,
            planes: Default::default(),
            is_disjoint: Default::default(),
        };
        dmabuf.planes.push(DmaBufPlane {
            offset: *udmabuf_offset as _,
            stride: stride as _,
            fd: udmabuf,
        });
        let img = match ctx.clone().dmabuf_img(&dmabuf) {
            Ok(i) => i,
            Err(e) => {
                *tex_impossible = true;
                log::debug!("Could not import udmabuf as GfxImage: {}", ErrorFmt(e));
                return false;
            }
        };
        let tex_ = match img.to_texture() {
            Ok(i) => i,
            Err(e) => {
                *tex_impossible = true;
                log::debug!("Could not import udmabuf as GfxTexture: {}", ErrorFmt(e));
                return false;
            }
        };
        *tex = Some(tex_);
        log::debug!("Using zero-copy wl_shm path");
        true
    }

    fn update_texture(&self, surface: &WlSurface, sync_shm: bool) -> Result<(), WlBufferError> {
        let storage = &mut *self.storage.borrow_mut();
        let storage = match storage {
            Some(s) => s,
            _ => return Ok(()),
        };
        match storage {
            WlBufferStorage::Shm {
                mem,
                stride,
                dmabuf_buffer_params,
                ..
            } => {
                if !sync_shm {
                    return Ok(());
                }
                let Some(ctx) = self.client.state.render_ctx.get() else {
                    return Ok(());
                };
                if ctx.fast_ram_access()
                    && self.import_udmabuf_texture(&ctx, mem, *stride, dmabuf_buffer_params)
                {
                    return Ok(());
                }
                let tex = ctx.async_shmem_texture(
                    self.format,
                    self.width,
                    self.height,
                    *stride,
                    &self.client.state.cpu_worker,
                )?;
                mem.access(|mem| tex.clone().sync_upload(mem, Region::new(self.rect)))??;
                surface.shm_textures.front().tex.set(Some(tex));
                surface.shm_textures.front().damage.clear();
            }
            WlBufferStorage::Dmabuf { img, tex, .. } => {
                if tex.is_none() {
                    *tex = Some(img.clone().to_texture()?);
                }
            }
        }
        Ok(())
    }

    pub fn update_framebuffer(&self) -> Result<(), WlBufferError> {
        let storage = &mut *self.storage.borrow_mut();
        let storage = match storage {
            Some(s) => s,
            _ => return Ok(()),
        };
        match storage {
            WlBufferStorage::Shm { .. } => {
                // nothing
            }
            WlBufferStorage::Dmabuf { img, fb, .. } => {
                if fb.is_none() {
                    *fb = Some(img.clone().to_framebuffer()?);
                }
            }
        }
        Ok(())
    }

    pub fn send_release(&self) {
        self.client.event(Release { self_id: self.id })
    }
}

impl WlBufferRequestHandler for WlBuffer {
    type Error = WlBufferError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        self.destroyed.set(true);
        Ok(())
    }
}

object_base! {
    self = WlBuffer;
    version = Version(1);
}

impl Object for WlBuffer {}

dedicated_add_obj!(WlBuffer, WlBufferId, buffers);

#[derive(Debug, Error)]
pub enum WlBufferError {
    #[error("The requested memory region is out of bounds for the pool")]
    OutOfBounds,
    #[error("The stride does not fit all pixels in a row")]
    StrideTooSmall,
    #[error("Could not access the client memory")]
    ClientMemError(#[source] Box<ClientMemError>),
    #[error("The graphics library could not import the client image")]
    GfxError(#[from] GfxError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlBufferError, ClientMemError);
efrom!(WlBufferError, ClientError);
