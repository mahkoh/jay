use {
    crate::{
        client::{Client, ClientError},
        clientmem::{ClientMem, ClientMemError, ClientMemOffset},
        format::Format,
        gfx_apis::gl::{Framebuffer, Image, RenderError, Texture},
        leaks::Tracker,
        object::Object,
        rect::Rect,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
            errorfmt::ErrorFmt,
        },
        video::dmabuf::DmaBuf,
        wire::{wl_buffer::*, WlBufferId},
    },
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
};

pub enum WlBufferStorage {
    Shm { mem: ClientMemOffset, stride: i32 },
    Dmabuf(Rc<Image>),
}

pub struct WlBuffer {
    pub id: WlBufferId,
    destroyed: Cell<bool>,
    pub client: Rc<Client>,
    pub rect: Rect,
    pub format: &'static Format,
    dmabuf: Option<DmaBuf>,
    render_ctx_version: Cell<u32>,
    pub storage: RefCell<Option<WlBufferStorage>>,
    pub texture: CloneCell<Option<Rc<Texture>>>,
    pub famebuffer: CloneCell<Option<Rc<Framebuffer>>>,
    width: i32,
    height: i32,
    pub tracker: Tracker<Self>,
}

impl WlBuffer {
    pub fn destroyed(&self) -> bool {
        self.destroyed.get()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_dmabuf(
        id: WlBufferId,
        client: &Rc<Client>,
        format: &'static Format,
        dmabuf: DmaBuf,
        img: &Rc<Image>,
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
            texture: CloneCell::new(None),
            famebuffer: Default::default(),
            dmabuf: Some(dmabuf),
            render_ctx_version: Cell::new(client.state.render_ctx_version.get()),
            storage: RefCell::new(Some(WlBufferStorage::Dmabuf(img.clone()))),
            tracker: Default::default(),
        }
    }

    #[allow(clippy::too_many_arguments)]
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
        let bytes = stride as u64 * height as u64;
        let required = bytes + offset as u64;
        if required > mem.len() as u64 {
            return Err(WlBufferError::OutOfBounds);
        }
        let mem = mem.offset(offset);
        let min_row_size = width as u64 * format.bpp as u64;
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
            storage: RefCell::new(Some(WlBufferStorage::Shm { mem, stride })),
            width,
            height,
            texture: CloneCell::new(None),
            tracker: Default::default(),
            famebuffer: Default::default(),
        })
    }

    pub fn handle_gfx_context_change(&self) {
        let ctx_version = self.client.state.render_ctx_version.get();
        if self.render_ctx_version.replace(ctx_version) == ctx_version {
            return;
        }
        self.texture.set(None);
        self.famebuffer.set(None);
        let mut storage = self.storage.borrow_mut();
        if let Some(storage) = &mut *storage {
            if let WlBufferStorage::Shm { .. } = storage {
                return;
            }
        }
        *storage = None;
        let ctx = match self.client.state.render_ctx.get() {
            Some(ctx) => ctx,
            _ => return,
        };
        if let Some(dmabuf) = &self.dmabuf {
            let image = match ctx.dmabuf_img(dmabuf) {
                Ok(image) => image,
                Err(e) => {
                    log::error!(
                        "Cannot re-import wl_buffer after graphics context reset: {}",
                        ErrorFmt(e)
                    );
                    return;
                }
            };
            *storage = Some(WlBufferStorage::Dmabuf(image));
        }
    }

    pub fn update_texture(&self) -> Result<(), WlBufferError> {
        let storage = self.storage.borrow_mut();
        let storage = match storage.deref() {
            Some(s) => s,
            _ => return Ok(()),
        };
        match storage {
            WlBufferStorage::Shm { mem, stride } => {
                self.texture.set(None);
                if let Some(ctx) = self.client.state.render_ctx.get() {
                    let tex = mem.access(|mem| {
                        ctx.shmem_texture(mem, self.format, self.width, self.height, *stride)
                    })??;
                    self.texture.set(Some(tex));
                }
            }
            WlBufferStorage::Dmabuf(img) => {
                if self.texture.get().is_none() {
                    self.texture.set(Some(img.to_texture()?));
                }
            }
        }
        Ok(())
    }

    pub fn update_framebuffer(&self) -> Result<(), WlBufferError> {
        let storage = self.storage.borrow_mut();
        let storage = match storage.deref() {
            Some(s) => s,
            _ => return Ok(()),
        };
        match storage {
            WlBufferStorage::Shm { .. } => {
                // nothing
            }
            WlBufferStorage::Dmabuf(img) => {
                if self.famebuffer.get().is_none() {
                    self.famebuffer.set(Some(img.to_framebuffer()?));
                }
            }
        }
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WlBufferError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        self.destroyed.set(true);
        Ok(())
    }

    pub fn send_release(&self) {
        self.client.event(Release { self_id: self.id })
    }
}

object_base! {
    WlBuffer;

    DESTROY => destroy,
}

impl Object for WlBuffer {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }
}

dedicated_add_obj!(WlBuffer, WlBufferId, buffers);

#[derive(Debug, Error)]
pub enum WlBufferError {
    #[error("The requested memory region is out of bounds for the pool")]
    OutOfBounds,
    #[error("The stride does not fit all pixels in a row")]
    StrideTooSmall,
    #[error("Could not access the client memory")]
    ClientMemError(#[source] Box<ClientMemError>),
    #[error("GLES could not import the client image")]
    RenderError(#[source] Box<RenderError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlBufferError, ClientMemError);
efrom!(WlBufferError, RenderError);
efrom!(WlBufferError, MsgParserError);
efrom!(WlBufferError, ClientError);
