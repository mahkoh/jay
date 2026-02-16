use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::{wl_region::WlRegion, wl_surface::WlSurface},
        leaks::Tracker,
        object::{Object, Version},
        wire::{WlCompositorId, wl_compositor::*},
        xwayland::XWaylandEvent,
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WlCompositorGlobal {
    name: GlobalName,
}

pub struct WlCompositor {
    id: WlCompositorId,
    client: Rc<Client>,
    version: Version,
    pub tracker: Tracker<Self>,
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
    ) -> Result<(), WlCompositorError> {
        let obj = Rc::new(WlCompositor {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlCompositorRequestHandler for WlCompositor {
    type Error = WlCompositorError;

    fn create_surface(&self, req: CreateSurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = Rc::new_cyclic(|slf| WlSurface::new(req.id, &self.client, self.version, slf));
        track!(self.client, surface);
        self.client.add_client_obj(&surface)?;
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
        self.client.add_client_obj(&region)?;
        Ok(())
    }
}

global_base!(WlCompositorGlobal, WlCompositor, WlCompositorError);

impl Global for WlCompositorGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        7
    }
}

simple_add_global!(WlCompositorGlobal);

object_base! {
    self = WlCompositor;
    version = self.version;
}

impl Object for WlCompositor {}

simple_add_obj!(WlCompositor);

#[derive(Debug, Error)]
pub enum WlCompositorError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(WlCompositorError, ClientError);
