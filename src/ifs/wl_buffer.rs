use crate::client::{Client, ClientError};
use crate::clientmem::{ClientMem, ClientMemError, ClientMemOffset};
use crate::format::Format;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::rect::Rect;
use crate::render::{Image, RenderError, Texture};
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::utils::clonecell::CloneCell;
use crate::wire::wl_buffer::*;
use crate::wire::WlBufferId;
use std::cell::Cell;
use std::rc::Rc;
use thiserror::Error;

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
    storage: WlBufferStorage,
    pub texture: CloneCell<Option<Rc<Texture>>>,
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
            storage: WlBufferStorage::Dmabuf(img.clone()),
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
            storage: WlBufferStorage::Shm { mem, stride },
            width,
            height,
            texture: CloneCell::new(None),
            tracker: Default::default(),
        })
    }

    pub fn update_texture(&self) -> Result<(), WlBufferError> {
        match &self.storage {
            WlBufferStorage::Shm { mem, stride } => {
                self.texture.set(None);
                let ctx = self.client.state.render_ctx.get().unwrap();
                let tex = mem.access(|mem| {
                    ctx.shmem_texture(mem, self.format, self.width, self.height, *stride)
                })??;
                self.texture.set(Some(tex));
            }
            WlBufferStorage::Dmabuf(img) => {
                if self.texture.get().is_none() {
                    self.texture.set(Some(img.to_texture()?));
                }
            }
        }
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
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
    WlBuffer, WlBufferError;

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
    #[error("Could not handle a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not access the client memory")]
    ClientMemError(#[source] Box<ClientMemError>),
    #[error("GLES could not import the client image")]
    GlesError(#[source] Box<RenderError>),
}
efrom!(WlBufferError, ClientMemError);
efrom!(WlBufferError, GlesError, RenderError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);
