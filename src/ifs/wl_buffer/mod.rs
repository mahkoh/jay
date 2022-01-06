mod types;

use crate::client::{AddObj, Client, DynEventFormatter};
use crate::clientmem::{ClientMem, ClientMemOffset};
use crate::format::Format;
use crate::ifs::wl_surface::{WlSurface, WlSurfaceId};
use crate::object::{Interface, Object, ObjectId};
use crate::pixman;
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;

const RELEASE: u32 = 0;

id!(WlBufferId);

pub struct WlBuffer {
    id: WlBufferId,
    client: Rc<Client>,
    offset: usize,
    pub width: u32,
    pub height: u32,
    stride: u32,
    format: &'static Format,
    pub image: Rc<pixman::Image<ClientMemOffset>>,
    pub(super) surfaces: CopyHashMap<WlSurfaceId, Rc<WlSurface>>,
}

impl WlBuffer {
    pub fn new(
        id: WlBufferId,
        client: &Rc<Client>,
        offset: usize,
        width: u32,
        height: u32,
        stride: u32,
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
        let image = pixman::Image::new(mem, format.pixman, width, height, stride)?;
        Ok(Self {
            id,
            client: client.clone(),
            offset,
            width,
            height,
            stride,
            format,
            image: Rc::new(image),
            surfaces: Default::default(),
        })
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        {
            let surfaces = self.surfaces.lock();
            for surface in surfaces.values() {
                *surface.buffer.borrow_mut() = None;
            }
        }
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlBufferError> {
        match request {
            DESTROY => self.destroy(parser).await?,
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

    fn break_loops(&self) {
        self.surfaces.clear();
    }
}
