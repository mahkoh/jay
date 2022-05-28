use {
    crate::{
        client::{Client, ClientError},
        clientmem::{ClientMem, ClientMemError},
        format::{formats, map_wayland_format_id},
        ifs::wl_buffer::{WlBuffer, WlBufferError},
        leaks::Tracker,
        object::Object,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{wl_shm_pool::*, WlShmPoolId},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct WlShmPool {
    id: WlShmPoolId,
    client: Rc<Client>,
    fd: Rc<OwnedFd>,
    mem: CloneCell<Rc<ClientMem>>,
    pub tracker: Tracker<Self>,
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
            tracker: Default::default(),
        })
    }

    fn create_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), WlShmPoolError> {
        let req: CreateBuffer = self.client.parse(self, parser)?;
        let drm_format = map_wayland_format_id(req.format);
        let format = match formats().get(&drm_format) {
            Some(f) if f.shm_supported => *f,
            _ => return Err(WlShmPoolError::InvalidFormat(req.format)),
        };
        if req.height < 0 || req.width < 0 || req.stride < 0 || req.offset < 0 {
            return Err(WlShmPoolError::NegativeParameters);
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
        track!(self.client, buffer);
        self.client.add_client_obj(&buffer)?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WlShmPoolError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn resize(&self, parser: MsgParser<'_, '_>) -> Result<(), WlShmPoolError> {
        let req: Resize = self.client.parse(self, parser)?;
        if req.size < 0 {
            return Err(WlShmPoolError::NegativeSize);
        }
        if (req.size as usize) < self.mem.get().len() {
            return Err(WlShmPoolError::CannotShrink);
        }
        self.mem
            .set(Rc::new(ClientMem::new(self.fd.raw(), req.size as usize)?));
        Ok(())
    }
}

object_base! {
    WlShmPool;

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
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("Tried to shrink the pool")]
    CannotShrink,
    #[error("Requested size is negative")]
    NegativeSize,
    #[error("Format {0} is not supported")]
    InvalidFormat(u32),
    #[error("All parameters in a create_buffer request must be non-negative")]
    NegativeParameters,
    #[error(transparent)]
    WlBufferError(Box<WlBufferError>),
}
efrom!(WlShmPoolError, ClientError);
efrom!(WlShmPoolError, ClientMemError);
efrom!(WlShmPoolError, WlBufferError);
efrom!(WlShmPoolError, MsgParserError);
