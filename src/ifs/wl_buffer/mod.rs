mod types;

use std::cell::Cell;
use crate::client::{Client, DynEventFormatter};
use crate::clientmem::{ClientMem, ClientMemOffset};
use crate::format::Format;
use crate::object::{Interface, Object, ObjectId};
use crate::rect::Rect;
use crate::render::{Image, Texture};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;

const RELEASE: u32 = 0;

id!(WlBufferId);

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

    fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlBufferError> {
        match request {
            DESTROY => self.destroy(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }

    pub fn release(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Release { obj: self.clone() })
    }
}

handle_request!(WlBuffer);

impl Object for WlBuffer {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlBuffer
    }

    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }
}
