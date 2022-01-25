mod types;

use crate::client::Client;
use crate::clientmem::ClientMem;
use crate::ifs::wl_buffer::WlBuffer;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use std::rc::Rc;
pub use types::*;
use uapi::OwnedFd;

const CREATE_BUFFER: u32 = 0;
const DESTROY: u32 = 1;
const RESIZE: u32 = 2;

id!(WlShmPoolId);

pub struct WlShmPool {
    id: WlShmPoolId,
    client: Rc<Client>,
    fd: OwnedFd,
    mem: CloneCell<Rc<ClientMem>>,
}

impl WlShmPool {
    pub fn new(
        id: WlShmPoolId,
        client: &Rc<Client>,
        fd: OwnedFd,
        len: usize,
    ) -> Result<Self, WlShmPoolError> {
        Ok(Self {
            id,
            client: client.clone(),
            mem: CloneCell::new(Rc::new(ClientMem::new(fd.raw(), len)?)),
            fd,
        })
    }

    fn create_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateBufferError> {
        let req: CreateBuffer = self.client.parse(self, parser)?;
        let format = match self.client.state.formats.get(&req.format) {
            Some(f) => *f,
            _ => return Err(CreateBufferError::InvalidFormat(req.format)),
        };
        if req.height < 0 || req.width < 0 || req.stride < 0 || req.offset < 0 {
            return Err(CreateBufferError::NegativeParameters);
        }
        let buffer = Rc::new(WlBuffer::new(
            req.id,
            &self.client,
            req.offset as usize,
            req.width,
            req.height,
            req.stride,
            format,
            &self.mem.get(),
        )?);
        self.client.add_client_obj(&buffer)?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn resize(&self, parser: MsgParser<'_, '_>) -> Result<(), ResizeError> {
        let req: Resize = self.client.parse(self, parser)?;
        if req.size < 0 {
            return Err(ResizeError::NegativeSize);
        }
        if (req.size as usize) < self.mem.get().len() {
            return Err(ResizeError::CannotShrink);
        }
        self.mem
            .set(Rc::new(ClientMem::new(self.fd.raw(), req.size as usize)?));
        Ok(())
    }

    fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlShmPoolError> {
        match request {
            CREATE_BUFFER => self.create_buffer(parser)?,
            DESTROY => self.destroy(parser)?,
            RESIZE => self.resize(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlShmPool);

impl Object for WlShmPool {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlShmPool
    }

    fn num_requests(&self) -> u32 {
        RESIZE + 1
    }
}
