use crate::client::{Client, ClientError};
use crate::globals::{Global, GlobalName};
use crate::ifs::org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
use thiserror::Error;
use crate::wire::org_kde_kwin_server_decoration_manager::*;
use crate::utils::buffd::MsgParserError;
use crate::wire::OrgKdeKwinServerDecorationManagerId;

#[allow(dead_code)]
const NONE: u32 = 0;
#[allow(dead_code)]
const CLIENT: u32 = 1;
const SERVER: u32 = 2;

pub struct OrgKdeKwinServerDecorationManagerGlobal {
    name: GlobalName,
}
impl OrgKdeKwinServerDecorationManagerGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: OrgKdeKwinServerDecorationManagerId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), OrgKdeKwinServerDecorationManagerError> {
        let obj = Rc::new(OrgKdeKwinServerDecorationManager {
            id,
            client: client.clone(),
            _version: version,
        });
        client.add_client_obj(&obj)?;
        obj.send_default_mode(SERVER);
        Ok(())
    }
}

global_base!(
    OrgKdeKwinServerDecorationManagerGlobal,
    OrgKdeKwinServerDecorationManager,
    OrgKdeKwinServerDecorationManagerError
);

impl Global for OrgKdeKwinServerDecorationManagerGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(OrgKdeKwinServerDecorationManagerGlobal);

pub struct OrgKdeKwinServerDecorationManager {
    id: OrgKdeKwinServerDecorationManagerId,
    client: Rc<Client>,
    _version: u32,
}

impl OrgKdeKwinServerDecorationManager {
    fn send_default_mode(self: &Rc<Self>, mode: u32) {
        self.client.event(DefaultMode {
            self_id: self.id,
            mode,
        })
    }

    fn create(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateError> {
        let req: Create = self.client.parse(self, parser)?;
        let _ = self.client.lookup(req.surface)?;
        let obj = Rc::new(OrgKdeKwinServerDecoration::new(req.id, &self.client));
        self.client.add_client_obj(&obj)?;
        obj.send_mode(SERVER);
        Ok(())
    }
}

object_base! {
    OrgKdeKwinServerDecorationManager, OrgKdeKwinServerDecorationManagerError;

    CREATE => create,
}

impl Object for OrgKdeKwinServerDecorationManager {
    fn num_requests(&self) -> u32 {
        CREATE + 1
    }
}

simple_add_obj!(OrgKdeKwinServerDecorationManager);

#[derive(Debug, Error)]
pub enum OrgKdeKwinServerDecorationManagerError {
    #[error("Could not process a `create` request")]
    CreateError(#[from] CreateError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(
    OrgKdeKwinServerDecorationManagerError,
    ClientError,
    ClientError
);

#[derive(Debug, Error)]
pub enum CreateError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(CreateError, ClientError);
efrom!(CreateError, ParseError, MsgParserError);
