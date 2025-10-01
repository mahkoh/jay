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
        utils::{errorfmt::ErrorFmt, page_size::page_size},
        video::dmabuf::DmaBuf,
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
    pub tracker: Tracker<Self>,
}

impl WlBuffer {
    pub fn destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub fn is_shm(&self) -> bool {
        self.shm
    }

    pub fn new_dmabuf(
        id: WlBufferId,
        client: &Rc<Client>,
        format: &'static Format,
        dmabuf: DmaBuf,
        img: &Rc<dyn GfxImage>,
    ) -> Self {
        let width = img.width();
        let height = img.height();
        Self {
            id,
            destroyed: Cell::new(false),
            client: client.clone(),
            rect: Rect::new_sized(0, 0, width, height).unwrap(),
            format,
            width,
            height,
            dmabuf: Some(dmabuf),
            render_ctx_version: Cell::new(client.state.render_ctx_version.get()),
            storage: RefCell::new(Some(WlBufferStorage::Dmabuf {
                img: img.clone(),
                tex: None,
                fb: None,
            })),
            shm: false,
            tracker: Default::default(),
            color: None,
        }
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
    ) -> Result<Self, WlBufferError> {
        let Some(shm_info) = &format.shm_info else {
            return Err(WlBufferError::UnsupportedShmFormat(format.name));
        };
        let bytes = stride as u64 * height as u64;
        let required = bytes + offset as u64;
        if required > mem.len() as u64 {
            return Err(WlBufferError::OutOfBounds);
        }
        let mem = Rc::new(mem.offset(offset));
        let min_row_size = width as u64 * shm_info.bpp as u64;
        if (stride as u64) < min_row_size {
            return Err(WlBufferError::StrideTooSmall);
        }
        Ok(Self {
            id,
            destroyed: Cell::new(false),
            client: client.clone(),
            rect: Rect::new_sized(0, 0, width, height).unwrap(),
            format,
            dmabuf: None,
            render_ctx_version: Cell::new(client.state.render_ctx_version.get()),
            storage: RefCell::new(Some(WlBufferStorage::Shm {
                dmabuf_buffer_params: DmabufBufferParams {
                    size: bytes as usize,
                    udmabuf: None,
                    udmabuf_offset: 0,
                    udmabuf_size: 0,
                    udmabuf_impossible: !mem.pool().is_sealed_memfd(),
                    host_buffer: None,
                    host_buffer_impossible: !mem.pool().is_sealed_memfd(),
                },
                mem,
                stride,
            })),
            shm: true,
            width,
            height,
            tracker: Default::default(),
            color: None,
        })
    }

    pub fn new_single_pixel(
        id: WlBufferId,
        client: &Rc<Client>,
        r: u32,
        g: u32,
        b: u32,
        a: u32,
    ) -> Self {
        Self {
            id,
            destroyed: Cell::new(false),
            client: client.clone(),
            rect: Rect::new_sized(0, 0, 1, 1).unwrap(),
            format: ARGB8888,
            dmabuf: None,
            render_ctx_version: Cell::new(client.state.render_ctx_version.get()),
            storage: RefCell::new(None),
            shm: false,
            width: 1,
            height: 1,
            tracker: Default::default(),
            color: Some([r, g, b, a]),
        }
    }

    pub fn handle_gfx_context_change(&self, surface: Option<&WlSurface>) {
        let ctx_version = self.client.state.render_ctx_version.get();
        if self.render_ctx_version.replace(ctx_version) == ctx_version {
            return;
        }
        let had_texture = self.reset_gfx_objects(surface);
        if had_texture && let Some(surface) = surface {
            self.update_texture_or_log(surface, true);
        }
    }

    fn reset_gfx_objects(&self, surface: Option<&WlSurface>) -> bool {
        let mut storage = self.storage.borrow_mut();
        let Some(s) = &mut *storage else {
            return false;
        };
        let had_texture = match s {
            WlBufferStorage::Shm {
                mem,
                dmabuf_buffer_params:
                    DmabufBufferParams {
                        host_buffer,
                        host_buffer_impossible,
                        ..
                    },
                ..
            } => {
                host_buffer.take();
                *host_buffer_impossible = !mem.pool().is_sealed_memfd();
                return match surface {
                    Some(s) => {
                        s.shm_staging.take();
                        s.shm_textures.back().tex.take();
                        s.shm_textures.front().tex.take().is_some()
                    }
                    None => false,
                };
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
                WlBufferStorage::Shm { .. } => {
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

    pub fn get_gfx_buffer(
        self: &Rc<Self>,
        ctx: &Rc<dyn GfxContext>,
        mem: &Rc<ClientMemOffset>,
        dmabuf_buffer_params: &mut DmabufBufferParams,
    ) -> Result<Option<Rc<dyn GfxBuffer>>, GfxError> {
        let DmabufBufferParams {
            size,
            udmabuf,
            udmabuf_offset,
            udmabuf_size,
            udmabuf_impossible,
            host_buffer,
            host_buffer_impossible,
        } = dmabuf_buffer_params;
        if let Some(hb) = host_buffer {
            return Ok(Some(hb.clone()));
        }
        if *host_buffer_impossible {
            return Ok(None);
        }
        let udmabuf = 'udmabuf: {
            if let Some(b) = udmabuf {
                break 'udmabuf b.clone();
            }
            if *udmabuf_impossible {
                return Ok(None);
            }
            let Some(dev) = self.client.state.udmabuf.get() else {
                return Ok(None);
            };
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
                    b
                }
                Err(e) => {
                    *udmabuf_impossible = true;
                    log::debug!("Could not create udmabuf: {}", ErrorFmt(e));
                    return Ok(None);
                }
            }
        };
        let hb = match ctx.create_dmabuf_buffer(&udmabuf, *udmabuf_offset, *udmabuf_size) {
            Ok(hb) => hb,
            Err(e) => {
                *host_buffer_impossible = true;
                log::debug!("Could not create gfx host buffer: {}", ErrorFmt(e));
                return Ok(None);
            }
        };
        *host_buffer = Some(hb.clone());
        Ok(Some(hb))
    }

    fn update_texture(&self, surface: &WlSurface, sync_shm: bool) -> Result<(), WlBufferError> {
        let storage = &mut *self.storage.borrow_mut();
        let storage = match storage {
            Some(s) => s,
            _ => return Ok(()),
        };
        match storage {
            WlBufferStorage::Shm { mem, stride, .. } => {
                if sync_shm && let Some(ctx) = self.client.state.render_ctx.get() {
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
    #[error("Buffer format {0} is not supported for shm buffers")]
    UnsupportedShmFormat(&'static str),
}
efrom!(WlBufferError, ClientMemError);
efrom!(WlBufferError, ClientError);
