mod types;

use crate::client::{AddObj, Client};
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_shm_pool::WlShmPool;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::WlParser;
use std::rc::Rc;
pub use types::*;

const CREATE_POOL: u32 = 0;

const FORMAT: u32 = 0;

pub struct WlShmGlobal {
    name: GlobalName,
}

pub struct WlShmObj {
    global: Rc<WlShmGlobal>,
    id: ObjectId,
    client: Rc<Client>,
}

impl WlShmGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    async fn bind_(
        self: Rc<Self>,
        id: ObjectId,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), WlShmError> {
        let obj = Rc::new(WlShmObj {
            global: self,
            id,
            client: client.clone(),
        });
        client.add_client_obj(&obj)?;
        for &format in Format::formats() {
            client
                .event(Box::new(FormatE {
                    obj: obj.clone(),
                    format,
                }))
                .await?;
        }
        Ok(())
    }
}

impl WlShmObj {
    async fn create_pool(&self, parser: WlParser<'_, '_>) -> Result<(), CreatePoolError> {
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

    async fn handle_request_(
        &self,
        request: u32,
        parser: WlParser<'_, '_>,
    ) -> Result<(), WlShmError> {
        match request {
            CREATE_POOL => self.create_pool(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Format {
    Argb8888,
    Xrgb8888,
}

impl Format {
    fn uint(self) -> u32 {
        match self {
            Format::Argb8888 => 0,
            Format::Xrgb8888 => 1,
        }
    }

    fn formats() -> &'static [Format] {
        &[Format::Argb8888, Format::Xrgb8888]
    }
}

bind!(WlShmGlobal);

impl Global for WlShmGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn interface(&self) -> Interface {
        Interface::WlShm
    }

    fn version(&self) -> u32 {
        1
    }

    fn pre_remove(&self) {
        unreachable!()
    }
}

handle_request!(WlShmObj);

impl Object for WlShmObj {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::WlShm
    }

    fn num_requests(&self) -> u32 {
        CREATE_POOL + 1
    }
}
