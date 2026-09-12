use crate::client::Client;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration;
use crate::ifs::org_kde_kwin_server_decoration::OrgKdeKwinServerDecorationError;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::OrgKdeKwinServerDecorationManagerId;
use crate::wire::org_kde_kwin_server_decoration_manager::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[expect(unused)]
const NONE: u32 = 0;
#[expect(unused)]
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
    ) -> Result<(), Infallible> {
        let obj = Rc::new(OrgKdeKwinServerDecorationManager {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        obj.send_default_mode(SERVER);
        Ok(())
    }
}

global_base!(
    OrgKdeKwinServerDecorationManagerGlobal,
    OrgKdeKwinServerDecorationManager,
);

impl Global for OrgKdeKwinServerDecorationManagerGlobal {
    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(OrgKdeKwinServerDecorationManagerGlobal);

#[derive(Object)]
pub struct OrgKdeKwinServerDecorationManager {
    id: OrgKdeKwinServerDecorationManagerId,
    client: Rc<Client>,
    version: Version,
    tracker: Tracker<Self>,
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
        self.client.add_client_obj(&obj);
        obj.send_mode(SERVER);
        Ok(())
    }
}
