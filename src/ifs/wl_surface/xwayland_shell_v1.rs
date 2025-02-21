use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::{WlSurfaceError, x_surface::xwayland_surface_v1::XwaylandSurfaceV1},
        leaks::Tracker,
        object::{Object, Version},
        wire::{WlSurfaceId, XwaylandShellV1Id, xwayland_shell_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct XwaylandShellV1Global {
    name: GlobalName,
}

pub struct XwaylandShellV1 {
    id: XwaylandShellV1Id,
    client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl XwaylandShellV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XwaylandShellV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), XwaylandShellV1Error> {
        let obj = Rc::new(XwaylandShellV1 {
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
impl XwaylandShellV1RequestHandler for XwaylandShellV1 {
    type Error = XwaylandShellV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_xwayland_surface(
        &self,
        req: GetXwaylandSurface,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let xsurface = surface.get_xsurface()?;
        if xsurface.xwayland_surface.is_some() {
            return Err(XwaylandShellV1Error::AlreadyAttached(surface.id));
        }
        let xws = Rc::new(XwaylandSurfaceV1 {
            id: req.id,
            client: self.client.clone(),
            x: xsurface,
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, xws);
        xws.x.xwayland_surface.set(Some(xws.clone()));
        self.client.add_client_obj(&xws)?;
        Ok(())
    }
}

global_base!(XwaylandShellV1Global, XwaylandShellV1, XwaylandShellV1Error);

impl Global for XwaylandShellV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn xwayland_only(&self) -> bool {
        true
    }
}

simple_add_global!(XwaylandShellV1Global);

object_base! {
    self = XwaylandShellV1;
    version = self.version;
}

impl Object for XwaylandShellV1 {}

simple_add_obj!(XwaylandShellV1);

#[derive(Debug, Error)]
pub enum XwaylandShellV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The `wl_surface` {0} already has an extension object")]
    AlreadyAttached(WlSurfaceId),
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
}
efrom!(XwaylandShellV1Error, ClientError);
