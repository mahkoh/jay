mod types;

use crate::client::{AddObj, Client};
use crate::clientmem::ClientMem;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::cell::RefCell;
use std::rc::Rc;
pub use types::*;
use uapi::OwnedFd;

const CREATE_BUFFER: u32 = 0;
const DESTROY: u32 = 1;
const RESIZE: u32 = 2;

pub struct WlShmPool {
    id: ObjectId,
    client: Rc<Client>,
    fd: OwnedFd,
    mem: RefCell<Rc<ClientMem>>,
}

impl WlShmPool {
    pub fn new(
        id: ObjectId,
        client: &Rc<Client>,
        fd: OwnedFd,
        len: usize,
    ) -> Result<Self, WlShmPoolError> {
        Ok(Self {
            id,
            client: client.clone(),
            mem: RefCell::new(Rc::new(ClientMem::new(fd.raw(), len)?)),
            fd,
        })
    }

    async fn create_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateBufferError> {
        let create: CreateBuffer = self.client.parse(self, parser)?;
        Ok(())
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _destroy: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn resize(&self, parser: MsgParser<'_, '_>) -> Result<(), ResizeError> {
        let resize: Resize = self.client.parse(self, parser)?;
        let mut mem = self.mem.borrow_mut();
        if resize.size < 0 {
            return Err(ResizeError::NegativeSize);
        }
        if (resize.size as usize) < mem.len() {
            return Err(ResizeError::CannotShrink);
        }
        *mem = Rc::new(ClientMem::new(self.fd.raw(), resize.size as usize)?);
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlShmPoolError> {
        match request {
            CREATE_BUFFER => self.create_buffer(parser).await?,
            DESTROY => self.destroy(parser).await?,
            RESIZE => self.resize(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlShmPool);

impl Object for WlShmPool {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::WlShmPool
    }

    fn num_requests(&self) -> u32 {
        RESIZE + 1
    }
}
