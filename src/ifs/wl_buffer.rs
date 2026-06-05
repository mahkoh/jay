use {
    crate::{
        client::{Client, ClientError},
        clientmem::{ClientMem, ClientMemError, ClientMemOffset},
        format::{ARGB8888, Format},
        gfx_api::{GfxBuffer, GfxContext, GfxError, GfxFramebuffer, GfxTexture},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        rect::{Rect, Region},
        state::DrmDevData,
        utils::{errorfmt::ErrorFmt, event_listener::EventListener, page_size::page_size},
        video::{
            LINEAR_MODIFIER,
            dmabuf::{DmaBuf, DmaBufPlane, PlaneVec},
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
    Dmabuf(WlBufferDmabufStorage),
}

pub struct WlBufferDmabufStorage {
    pub dmabuf: Rc<DmaBuf>,
    pub tex: Option<Rc<dyn GfxTexture>>,
    pub fb: Option<Rc<dyn GfxFramebuffer>>,
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

pub struct AttachedBuffer {
    pub send_release: bool,
    pub buf: Rc<WlBuffer>,
}

impl Drop for AttachedBuffer {
    fn drop(&mut self) {
        if self.send_release && !self.buf.destroyed() {
            self.buf.send_release();
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
enum Ty {
    Shm,
    DmaBuf,
    Spb,
}

pub struct WlBuffer {
    pub id: WlBufferId,
    destroyed: Cell<bool>,
    pub client: Rc<Client>,
    pub rect: Rect,
    pub format: &'static Format,
    pub client_dmabuf: Option<Rc<DmaBuf>>,
    #[expect(dead_code)]
    pub client_dmabuf_device: Option<Rc<DrmDevData>>,
    render_ctx_version: Cell<u32>,
    pub storage: RefCell<Option<WlBufferStorage>>,
    ty: Ty,
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
        self.ty == Ty::Shm
    }

    fn new(
        id: WlBufferId,
        client: &Rc<Client>,
        format: &'static Format,
        width: i32,
        height: i32,
        client_dmabuf: Option<Rc<DmaBuf>>,
        client_dmabuf_device: Option<Rc<DrmDevData>>,
        storage: Option<WlBufferStorage>,
        ty: Ty,
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
            client_dmabuf,
            client_dmabuf_device,
            render_ctx_version: Cell::new(client.state.render_ctx_version.get()),
            storage: RefCell::new(storage),
            ty,
            tracker: Default::default(),
            color,
            gfx_ctx_changed: EventListener::new(slf.clone()),
        });
        slf.gfx_ctx_changed.attach(&client.state.gfx_ctx_changed);
        slf
    }

    pub fn new_dmabuf(
        id: WlBufferId,
        client: &Rc<Client>,
        format: &'static Format,
        client_dmabuf: Rc<DmaBuf>,
    ) -> Rc<Self> {
        let device = client.state.find_dmabuf_device(&client_dmabuf);
        Self::new(
            id,
            client,
            format,
            client_dmabuf.width,
            client_dmabuf.height,
            Some(client_dmabuf.clone()),
            device,
            Some(WlBufferStorage::Dmabuf(WlBufferDmabufStorage {
                dmabuf: client_dmabuf,
                tex: None,
                fb: None,
            })),
            Ty::DmaBuf,
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
        client_dmabuf: Option<Rc<DmaBuf>>,
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
        let size = bytes as usize;
        let mem = Rc::new(mem.offset(offset, size));
        let min_row_size = width as u64 * format.bpp as u64;
        if (stride as u64) < min_row_size {
            return Err(WlBufferError::StrideTooSmall);
        }
        let udmabuf_impossible = !mem.pool().is_sealed_memfd();
        let dmabuf_buffer_params = match udmabuf {
            None => DmabufBufferParams {
                size,
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
            client_dmabuf,
            None,
            Some(WlBufferStorage::Shm {
                dmabuf_buffer_params,
                mem,
                stride,
            }),
            Ty::Shm,
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
            None,
            Ty::Spb,
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
                WlBufferStorage::Dmabuf(storage) => &storage.tex,
            };
            return tex.is_some();
        }
        match s {
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
            WlBufferStorage::Dmabuf(storage) => {
                let had_texture = storage.tex.is_some();
                storage.tex = None;
                storage.fb = None;
                had_texture
            }
        }
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
                WlBufferStorage::Dmabuf(storage) => storage.tex.clone(),
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
    ) -> Option<Rc<dyn GfxTexture>> {
        let DmabufBufferParams {
            tex,
            tex_impossible,
            ..
        } = dmabuf_buffer_params;
        if tex.is_some() {
            return tex.clone();
        }
        if *tex_impossible {
            return None;
        }
        let udmabuf = self.get_udmabuf(mem, dmabuf_buffer_params)?;
        let DmabufBufferParams {
            udmabuf_offset,
            tex,
            tex_impossible,
            ..
        } = dmabuf_buffer_params;
        let mut planes = PlaneVec::new();
        planes.push(DmaBufPlane {
            offset: *udmabuf_offset as _,
            stride: stride as _,
            fd: udmabuf,
        });
        let dmabuf = DmaBuf::new(
            &self.client.state.dma_buf_ids,
            self.width,
            self.height,
            self.format,
            LINEAR_MODIFIER,
            planes,
        );
        let tex_ = match ctx.clone().dmabuf_tex(&dmabuf) {
            Ok(i) => i,
            Err(e) => {
                *tex_impossible = true;
                log::debug!("Could not import udmabuf as GfxTexture: {}", ErrorFmt(e));
                return None;
            }
        };
        *tex = Some(tex_.clone());
        log::debug!("Using zero-copy wl_shm path");
        Some(tex_)
    }

    fn update_texture(&self, surface: &WlSurface, sync_shm: bool) -> Result<(), WlBufferError> {
        let storage = &mut *self.storage.borrow_mut();
        let storage = match storage {
            Some(s) => s,
            _ => return Ok(()),
        };
        let Some(ctx) = self.client.state.render_ctx.get() else {
            return Ok(());
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
                if ctx.fast_ram_access()
                    && self
                        .import_udmabuf_texture(&ctx, mem, *stride, dmabuf_buffer_params)
                        .is_some()
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
            WlBufferStorage::Dmabuf(storage) => {
                storage.ensure_tex(&ctx)?;
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
            WlBufferStorage::Dmabuf(storage) => {
                if let Some(ctx) = self.client.state.render_ctx.get() {
                    storage.ensure_fb(&ctx)?;
                }
            }
        }
        Ok(())
    }

    fn send_release(&self) {
        self.client.event(Release { self_id: self.id })
    }
}

impl WlBufferDmabufStorage {
    pub fn ensure_tex(
        &mut self,
        ctx: &Rc<dyn GfxContext>,
    ) -> Result<Rc<dyn GfxTexture>, WlBufferError> {
        if let Some(tex) = &self.tex {
            return Ok(tex.clone());
        }
        let tex = ctx.clone().dmabuf_tex(&self.dmabuf)?;
        Ok(self.tex.insert(tex).clone())
    }

    pub fn ensure_fb(
        &mut self,
        ctx: &Rc<dyn GfxContext>,
    ) -> Result<Rc<dyn GfxFramebuffer>, WlBufferError> {
        if let Some(fb) = &self.fb {
            return Ok(fb.clone());
        }
        let fb = ctx.clone().dmabuf_fb(&self.dmabuf)?;
        Ok(self.fb.insert(fb).clone())
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
