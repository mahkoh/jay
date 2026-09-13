use crate::client::Client;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_region::WlRegion;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WlCompositorId;
use crate::wire::wl_compositor::*;
use crate::xwayland::XWaylandEvent;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct WlCompositorGlobal {
    name: GlobalName,
}

#[derive(Object)]
pub struct WlCompositor {
    id: WlCompositorId,
    client: Rc<Client>,
    version: Version,
    tracker: Tracker<Self>,
}

impl WlCompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlCompositorId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(WlCompositor {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl WlCompositorRequestHandler for WlCompositor {
    type Error = Infallible;

    fn create_surface(&self, req: CreateSurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = Rc::new_cyclic(|slf| WlSurface::new(req.id, &self.client, self.version, slf));
        track!(self.client, surface);
        self.client.add_client_obj(&surface);
        if self.client.is_xwayland {
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::SurfaceCreated(surface.id));
        }
        Ok(())
    }

    fn create_region(&self, req: CreateRegion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let region = Rc::new(WlRegion::new(req.id, &self.client, self.version));
        track!(self.client, region);
        self.client.add_client_obj(&region);
        Ok(())
    }

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

impl Global for WlCompositorGlobal {
    fn version(&self) -> u32 {
        7
    }
}
