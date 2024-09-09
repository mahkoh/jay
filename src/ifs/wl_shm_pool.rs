use {
    crate::{
        client::{Client, ClientError},
        clientmem::{ClientMem, ClientMemError},
        format::{formats, map_wayland_format_id},
        ifs::wl_buffer::{WlBuffer, WlBufferError},
        leaks::Tracker,
        object::{Object, Version},
        utils::clonecell::CloneCell,
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
    version: Version,
}

impl WlShmPool {
    pub fn new(
        id: WlShmPoolId,
        client: &Rc<Client>,
        fd: Rc<OwnedFd>,
        len: usize,
        version: Version,
    ) -> Result<Self, WlShmPoolError> {
        Ok(Self {
            id,
            client: client.clone(),
            mem: CloneCell::new(Rc::new(ClientMem::new(
                &fd,
                len,
                false,
                Some(client),
                Some(&client.state.cpu_worker),
            )?)),
            fd,
            tracker: Default::default(),
            version,
        })
    }
}

impl WlShmPoolRequestHandler for WlShmPool {
    type Error = WlShmPoolError;

    fn create_buffer(&self, req: CreateBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let drm_format = map_wayland_format_id(req.format);
        let format = match formats().get(&drm_format) {
            Some(f) if f.shm_info.is_some() => *f,
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

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn resize(&self, req: Resize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.size < 0 {
            return Err(WlShmPoolError::NegativeSize);
        }
        if (req.size as usize) < self.mem.get().len() {
            return Err(WlShmPoolError::CannotShrink);
        }
        self.mem.set(Rc::new(ClientMem::new(
            &self.fd,
            req.size as usize,
            false,
            Some(&self.client),
            Some(&self.client.state.cpu_worker),
        )?));
        Ok(())
    }
}

object_base! {
    self = WlShmPool;
    version = self.version;
}

impl Object for WlShmPool {}

simple_add_obj!(WlShmPool);

#[derive(Debug, Error)]
pub enum WlShmPoolError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
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
