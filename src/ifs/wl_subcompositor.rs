use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wl_subsurface::{WlSubsurface, WlSubsurfaceError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wl_subcompositor::*, WlSubcompositorId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[expect(dead_code)]
const BAD_SURFACE: u32 = 0;

pub struct WlSubcompositorGlobal {
    name: GlobalName,
}

pub struct WlSubcompositor {
    id: WlSubcompositorId,
    client: Rc<Client>,
    pub tracker: Tracker<Self>,
    version: Version,
}

impl WlSubcompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlSubcompositorId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WlSubcompositorError> {
        let obj = Rc::new(WlSubcompositor {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlSubcompositorRequestHandler for WlSubcompositor {
    type Error = WlSubcompositorError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_subsurface(&self, req: GetSubsurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let parent = self.client.lookup(req.parent)?;
        let subsurface = Rc::new(WlSubsurface::new(req.id, &surface, &parent, self.version));
        track!(self.client, subsurface);
        self.client.add_client_obj(&subsurface)?;
        subsurface.install()?;
        Ok(())
    }
}

global_base!(WlSubcompositorGlobal, WlSubcompositor, WlSubcompositorError);

impl Global for WlSubcompositorGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WlSubcompositorGlobal);

object_base! {
    self = WlSubcompositor;
    version = self.version;
}

impl Object for WlSubcompositor {}

simple_add_obj!(WlSubcompositor);

#[derive(Debug, Error)]
pub enum WlSubcompositorError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSubsurfaceError(Box<WlSubsurfaceError>),
}
efrom!(WlSubcompositorError, ClientError);
efrom!(WlSubcompositorError, WlSubsurfaceError);
