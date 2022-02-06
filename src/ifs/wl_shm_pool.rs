use crate::client::{Client, ClientError};
use crate::clientmem::ClientMem;
use crate::format::{formats, map_wayland_format_id};
use crate::ifs::wl_buffer::{WlBuffer, WlBufferError};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::utils::clonecell::CloneCell;
use crate::wire::wl_shm_pool::*;
use crate::wire::WlShmPoolId;
use crate::ClientMemError;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

pub struct WlShmPool {
    id: WlShmPoolId,
    client: Rc<Client>,
    fd: Rc<OwnedFd>,
    mem: CloneCell<Rc<ClientMem>>,
}

impl WlShmPool {
    pub fn new(
        id: WlShmPoolId,
        client: &Rc<Client>,
        fd: Rc<OwnedFd>,
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
        let drm_format = map_wayland_format_id(req.format);
        let format = match formats().get(&drm_format) {
            Some(f) => *f,
            _ => return Err(CreateBufferError::InvalidFormat(req.format)),
        };
        if req.height < 0 || req.width < 0 || req.stride < 0 || req.offset < 0 {
            return Err(CreateBufferError::NegativeParameters);
        }
        let buffer = Rc::new(WlBuffer::new_shm(
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
}

object_base! {
    WlShmPool, WlShmPoolError;

    CREATE_BUFFER => create_buffer,
    DESTROY => destroy,
    RESIZE => resize,
}

impl Object for WlShmPool {
    fn num_requests(&self) -> u32 {
        RESIZE + 1
    }
}

simple_add_obj!(WlShmPool);

#[derive(Debug, Error)]
pub enum WlShmPoolError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `create_buffer` request")]
    CreateBufferError(#[from] CreateBufferError),
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `resize` request")]
    ResizeError(#[from] ResizeError),
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
}
efrom!(WlShmPoolError, ClientError);
efrom!(WlShmPoolError, ClientMemError);

#[derive(Debug, Error)]
pub enum CreateBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Format {0} is not supported")]
    InvalidFormat(u32),
    #[error("All parameters in a create_buffer request must be non-negative")]
    NegativeParameters,
    #[error(transparent)]
    WlBufferError(Box<WlBufferError>),
}
efrom!(CreateBufferError, ParseError, MsgParserError);
efrom!(CreateBufferError, ClientError);
efrom!(CreateBufferError, WlBufferError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseError, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum ResizeError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Tried to shrink the pool")]
    CannotShrink,
    #[error("Requested size is negative")]
    NegativeSize,
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
}
efrom!(ResizeError, ParseError, MsgParserError);
efrom!(ResizeError, ClientMemError);
