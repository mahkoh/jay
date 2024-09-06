use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::org_kde_kwin_server_decoration::{
            OrgKdeKwinServerDecoration, OrgKdeKwinServerDecorationError,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{org_kde_kwin_server_decoration_manager::*, OrgKdeKwinServerDecorationManagerId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[expect(dead_code)]
const NONE: u32 = 0;
#[expect(dead_code)]
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
            version,
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
    version: Version,
    pub tracker: Tracker<Self>,
}

impl OrgKdeKwinServerDecorationManager {
    fn send_default_mode(self: &Rc<Self>, mode: u32) {
        self.client.event(DefaultMode {
            self_id: self.id,
            mode,
        })
    }
}

impl OrgKdeKwinServerDecorationManagerRequestHandler for OrgKdeKwinServerDecorationManager {
    type Error = OrgKdeKwinServerDecorationError;

    fn create(&self, req: Create, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = self.client.lookup(req.surface)?;
        let obj = Rc::new(OrgKdeKwinServerDecoration::new(
            req.id,
            &self.client,
            self.version,
        ));
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_mode(SERVER);
        Ok(())
    }
}

object_base! {
    self = OrgKdeKwinServerDecorationManager;
    version = self.version;
}

impl Object for OrgKdeKwinServerDecorationManager {}

simple_add_obj!(OrgKdeKwinServerDecorationManager);

#[derive(Debug, Error)]
pub enum OrgKdeKwinServerDecorationManagerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(OrgKdeKwinServerDecorationManagerError, ClientError);
