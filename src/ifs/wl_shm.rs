use {
    crate::{
        client::{Client, ClientError},
        format::FORMATS,
        globals::{Global, GlobalName},
        ifs::wl_shm_pool::{WlShmPool, WlShmPoolError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wl_shm::*, WlShmId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WlShmGlobal {
    name: GlobalName,
}

pub struct WlShm {
    _global: Rc<WlShmGlobal>,
    id: WlShmId,
    client: Rc<Client>,
    version: Version,
    pub tracker: Tracker<Self>,
}

impl WlShmGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlShmId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WlShmError> {
        let obj = Rc::new(WlShm {
            _global: self,
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        for format in FORMATS {
            if format.shm_info.is_some() {
                client.event(Format {
                    self_id: id,
                    format: format.wl_id.unwrap_or(format.drm),
                });
            }
        }
        Ok(())
    }
}

impl WlShmRequestHandler for WlShm {
    type Error = WlShmError;

    fn create_pool(&self, create: CreatePool, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if create.size < 0 {
            return Err(WlShmError::NegativeSize);
        }
        let pool = Rc::new(WlShmPool::new(
            create.id,
            &self.client,
            create.fd,
            create.size as usize,
            self.version,
        )?);
        track!(self.client, pool);
        self.client.add_client_obj(&pool)?;
        Ok(())
    }

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

global_base!(WlShmGlobal, WlShm, WlShmError);

impl Global for WlShmGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        2
    }
}

simple_add_global!(WlShmGlobal);

object_base! {
    self = WlShm;
    version = self.version;
}

impl Object for WlShm {}

simple_add_obj!(WlShm);

#[derive(Debug, Error)]
pub enum WlShmError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The passed size is negative")]
    NegativeSize,
    #[error(transparent)]
    WlShmPoolError(Box<WlShmPoolError>),
}
efrom!(WlShmError, ClientError);
efrom!(WlShmError, WlShmPoolError);
