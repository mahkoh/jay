use crate::client::Client;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::jay_wl_surface_factory_v1::JayWlSurfaceFactoryV1;
use crate::ifs::wl_compositor::WL_COMPOSITOR_VERSION;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayWlSurfaceFactoryManagerV1Id;
use crate::wire::jay_wl_surface_factory_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct JayWlSurfaceFactoryManagerV1Global {
    pub name: GlobalName,
}

impl JayWlSurfaceFactoryManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayWlSurfaceFactoryManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(JayWlSurfaceFactoryManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

#[derive(Object)]
pub struct JayWlSurfaceFactoryManagerV1 {
    pub id: JayWlSurfaceFactoryManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayWlSurfaceFactoryManagerV1RequestHandler for JayWlSurfaceFactoryManagerV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn create_factory(&self, req: CreateFactory, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = JayWlSurfaceFactoryV1::new(req.id, self.version, &self.client);
        self.client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for JayWlSurfaceFactoryManagerV1Global {
    fn version(&self) -> u32 {
        WL_COMPOSITOR_VERSION
    }
}
