mod types;

use std::cell::RefCell;
use crate::client::{AddObj, Client};
use crate::clientmem::ClientMem;
use crate::format::Format;
use crate::object::{Interface, Object, ObjectId};
use std::rc::Rc;
pub use types::*;
use crate::utils::buffd::MsgParser;

const DESTROY: u32 = 0;

const RELEASE: u32 = 0;

id!(WlBufferId);

pub struct WlBuffer {
    id: WlBufferId,
    client: Rc<Client>,
    offset: usize,
    width: u32,
    height: u32,
    stride: u32,
    format: &'static Format,
    mem: RefCell<Option<Rc<ClientMem>>>,
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
        let min_row_size = width as u64 * format.bpp as u64;
        if (stride as u64) < min_row_size {
            return Err(WlBufferError::StrideTooSmall);
        }
        Ok(Self {
            id,
            client: client.clone(),
            offset,
            width,
            height,
            stride,
            format,
            mem: RefCell::new(Some(mem.clone())),
        })
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        *self.mem.borrow_mut() = None;
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn handle_request_(&self, request: u32, parser: MsgParser<'_, '_>) -> Result<(), WlBufferError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
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
