use {
    crate::{
        client::{Client, ClientError},
        format::FORMATS,
        globals::{Global, GlobalName},
        ifs::wl_shm_pool::{WlShmPool, WlShmPoolError},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
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
        _version: u32,
    ) -> Result<(), WlShmError> {
        let obj = Rc::new(WlShm {
            _global: self,
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        for format in FORMATS {
            client.event(Format {
                self_id: id,
                format: format.wl_id.unwrap_or(format.drm),
            });
        }
        Ok(())
    }
}

impl WlShm {
    fn create_pool(&self, parser: MsgParser<'_, '_>) -> Result<(), WlShmError> {
        let create: CreatePool = self.client.parse(self, parser)?;
        if create.size < 0 {
            return Err(WlShmError::NegativeSize);
        }
        let pool = Rc::new(WlShmPool::new(
            create.id,
            &self.client,
            create.fd,
            create.size as usize,
        )?);
        track!(self.client, pool);
        self.client.add_client_obj(&pool)?;
        Ok(())
    }
}

global_base!(WlShmGlobal, WlShm, WlShmError);

impl Global for WlShmGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WlShmGlobal);

object_base! {
    WlShm;

    CREATE_POOL => create_pool,
}

impl Object for WlShm {
    fn num_requests(&self) -> u32 {
        CREATE_POOL + 1
    }
}

simple_add_obj!(WlShm);

#[derive(Debug, Error)]
pub enum WlShmError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("The passed size is negative")]
    NegativeSize,
    #[error(transparent)]
    WlShmPoolError(Box<WlShmPoolError>),
}
efrom!(WlShmError, ClientError);
efrom!(WlShmError, MsgParserError);
efrom!(WlShmError, WlShmPoolError);
