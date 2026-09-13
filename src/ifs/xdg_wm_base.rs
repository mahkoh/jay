use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::ifs::wl_surface::xdg_surface::XdgSurfaceError;
use crate::ifs::xdg_positioner::XdgPositioner;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::utils::copyhashmap::CopyHashMap;
use crate::wire::XdgSurfaceId;
use crate::wire::XdgWmBaseId;
use crate::wire::xdg_wm_base::*;
use jay_proc::Global;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[expect(unused)]
const ROLE: u32 = 0;
const DEFUNCT_SURFACES: u32 = 1;
#[expect(unused)]
const NOT_THE_TOPMOST_POPUP: u32 = 2;
#[expect(unused)]
const INVALID_POPUP_PARENT: u32 = 3;
#[expect(unused)]
const INVALID_SURFACE_STATE: u32 = 4;
#[expect(unused)]
const INVALID_POSITIONER: u32 = 5;

#[derive(Global)]
pub struct XdgWmBaseGlobal {
    name: GlobalName,
}

#[derive(Object)]
#[break_loops]
pub struct XdgWmBase {
    id: XdgWmBaseId,
    client: Rc<Client>,
    pub version: Version,
    pub(super) surfaces: CopyHashMap<XdgSurfaceId, Rc<XdgSurface>>,
    tracker: Tracker<Self>,
}

impl XdgWmBaseGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgWmBaseId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), XdgWmBaseError> {
        let obj = Rc::new(XdgWmBase {
            id,
            client: client.clone(),
            version,
            surfaces: Default::default(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl XdgWmBaseRequestHandler for XdgWmBase {
    type Error = XdgWmBaseError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if !self.surfaces.is_empty() {
            self.client.protocol_error(
                self,
                DEFUNCT_SURFACES,
                &format!(
                    "Cannot destroy xdg_wm_base object {} before destroying its surfaces",
                    self.id
                ),
            );
            return Err(XdgWmBaseError::DefunctSurfaces);
        }
        self.client.remove_obj(self);
        Ok(())
    }

    fn create_positioner(&self, req: CreatePositioner, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pos = Rc::new(XdgPositioner::new(slf, req.id, &self.client));
        track!(self.client, pos);
        self.client.add_client_obj(&pos);
        Ok(())
    }

    fn get_xdg_surface(&self, req: GetXdgSurface, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let xdg_surface = Rc::new(XdgSurface::new(slf, req.id, &surface));
        track!(self.client, xdg_surface);
        self.client.add_client_obj(&xdg_surface);
        xdg_surface.install()?;
        self.surfaces.set(req.id, xdg_surface);
        Ok(())
    }

    fn pong(&self, _req: Pong, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Global for XdgWmBaseGlobal {
    fn version(&self) -> u32 {
        7
    }
}

impl BreakLoops for XdgWmBase {
    fn break_loops(self: Rc<Self>) {
        self.surfaces.clear();
    }
}

#[derive(Debug, Error)]
pub enum XdgWmBaseError {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error("Tried to destroy xdg_wm_base object before destroying its surfaces")]
    DefunctSurfaces,
    #[error(transparent)]
    XdgSurfaceError(Box<XdgSurfaceError>),
}
efrom!(XdgWmBaseError, XdgSurfaceError);
