use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::WlSurfaceError;
use crate::ifs::wl_surface::x_surface::xwayland_surface_v1::XwaylandSurfaceV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WlSurfaceId;
use crate::wire::XwaylandShellV1Id;
use crate::wire::xwayland_shell_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Global)]
pub struct XwaylandShellV1Global {
    name: GlobalName,
}

#[derive(Object)]
pub struct XwaylandShellV1 {
    id: XwaylandShellV1Id,
    client: Rc<Client>,
    version: Version,
    tracker: Tracker<Self>,
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
        client.add_client_obj(&obj);
        Ok(())
    }
}
impl XwaylandShellV1RequestHandler for XwaylandShellV1 {
    type Error = XwaylandShellV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
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
        self.client.add_client_obj(&xws);
        Ok(())
    }
}

impl Global for XwaylandShellV1Global {
    fn version(&self) -> u32 {
        1
    }

    fn xwayland_only(&self) -> bool {
        true
    }
}

#[derive(Debug, Error)]
pub enum XwaylandShellV1Error {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error("The `wl_surface` {0} already has an extension object")]
    AlreadyAttached(WlSurfaceId),
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
}
