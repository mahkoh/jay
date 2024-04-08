use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::org_kde_kwin_server_decoration::{
            OrgKdeKwinServerDecoration, OrgKdeKwinServerDecorationError,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::buffd::{MsgParser, MsgParserError},
        wire::{org_kde_kwin_server_decoration_manager::*, OrgKdeKwinServerDecorationManagerId},
    },
    std::rc::Rc,
    thiserror::Error,
};

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
        version: Version,
    ) -> Result<(), OrgKdeKwinServerDecorationManagerError> {
        let obj = Rc::new(OrgKdeKwinServerDecorationManager {
            id,
            client: client.clone(),
            _version: version,
            tracker: Default::default(),
        });
        track!(client, obj);
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
    _version: Version,
    pub tracker: Tracker<Self>,
}

impl OrgKdeKwinServerDecorationManager {
    fn send_default_mode(self: &Rc<Self>, mode: u32) {
        self.client.event(DefaultMode {
            self_id: self.id,
            mode,
        })
    }

    fn create(&self, parser: MsgParser<'_, '_>) -> Result<(), OrgKdeKwinServerDecorationError> {
        let req: Create = self.client.parse(self, parser)?;
        let _ = self.client.lookup(req.surface)?;
        let obj = Rc::new(OrgKdeKwinServerDecoration::new(req.id, &self.client));
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_mode(SERVER);
        Ok(())
    }
}

object_base! {
    self = OrgKdeKwinServerDecorationManager;

    CREATE => create,
}

impl Object for OrgKdeKwinServerDecorationManager {}

simple_add_obj!(OrgKdeKwinServerDecorationManager);

#[derive(Debug, Error)]
pub enum OrgKdeKwinServerDecorationManagerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(OrgKdeKwinServerDecorationManagerError, ClientError);
efrom!(OrgKdeKwinServerDecorationManagerError, MsgParserError);
