use crate::client::{Client, ClientError};
use crate::format::FORMATS;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_shm_pool::{WlShmPool, WlShmPoolError};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_shm::*;
use crate::wire::WlShmId;
use std::rc::Rc;
use thiserror::Error;

pub struct WlShmGlobal {
    name: GlobalName,
}

pub struct WlShm {
    _global: Rc<WlShmGlobal>,
    id: WlShmId,
    client: Rc<Client>,
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
        });
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
    fn create_pool(&self, parser: MsgParser<'_, '_>) -> Result<(), CreatePoolError> {
        let create: CreatePool = self.client.parse(self, parser)?;
        if create.size < 0 {
            return Err(CreatePoolError::NegativeSize);
        }
        let pool = Rc::new(WlShmPool::new(
            create.id,
            &self.client,
            create.fd,
            create.size as usize,
        )?);
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
    WlShm, WlShmError;

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
    #[error("Could not process a `create_pool` request")]
    CreatePoolError(#[from] CreatePoolError),
}
efrom!(WlShmError, ClientError);

#[derive(Debug, Error)]
pub enum CreatePoolError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("The passed size is negative")]
    NegativeSize,
    #[error(transparent)]
    WlShmPoolError(Box<WlShmPoolError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreatePoolError, ParseError, MsgParserError);
efrom!(CreatePoolError, WlShmPoolError);
efrom!(CreatePoolError, ClientError);
