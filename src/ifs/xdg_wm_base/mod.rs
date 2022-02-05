mod types;

use crate::client::Client;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceId};
use crate::ifs::xdg_positioner::XdgPositioner;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const CREATE_POSITIONER: u32 = 1;
const GET_XDG_SURFACE: u32 = 2;
const PONG: u32 = 3;

const PING: u32 = 0;

#[allow(dead_code)]
const ROLE: u32 = 0;
const DEFUNCT_SURFACES: u32 = 1;
#[allow(dead_code)]
const NOT_THE_TOPMOST_POPUP: u32 = 2;
#[allow(dead_code)]
const INVALID_POPUP_PARENT: u32 = 3;
#[allow(dead_code)]
const INVALID_SURFACE_STATE: u32 = 4;
#[allow(dead_code)]
const INVALID_POSITIONER: u32 = 5;

id!(XdgWmBaseId);

pub struct XdgWmBaseGlobal {
    name: GlobalName,
}

pub struct XdgWmBase {
    id: XdgWmBaseId,
    client: Rc<Client>,
    pub version: u32,
    pub(super) surfaces: CopyHashMap<XdgSurfaceId, Rc<XdgSurface>>,
}

impl XdgWmBaseGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgWmBaseId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), XdgWmBaseError> {
        let obj = Rc::new(XdgWmBase {
            id,
            client: client.clone(),
            version,
            surfaces: Default::default(),
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl XdgWmBase {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        if !self.surfaces.is_empty() {
            self.client.protocol_error(
                self,
                DEFUNCT_SURFACES,
                format!(
                    "Cannot destroy xdg_wm_base object {} before destroying its surfaces",
                    self.id
                ),
            );
            return Err(DestroyError::DefunctSurfaces);
        }
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn create_positioner(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), CreatePositionerError> {
        let req: CreatePositioner = self.client.parse(&**self, parser)?;
        let pos = Rc::new(XdgPositioner::new(self, req.id, &self.client));
        self.client.add_client_obj(&pos)?;
        Ok(())
    }

    fn get_xdg_surface(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), GetXdgSurfaceError> {
        let req: GetXdgSurface = self.client.parse(&**self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let xdg_surface = Rc::new(XdgSurface::new(self, req.id, &surface));
        self.client.add_client_obj(&xdg_surface)?;
        xdg_surface.install()?;
        self.surfaces.set(req.id, xdg_surface);
        Ok(())
    }

    fn pong(&self, parser: MsgParser<'_, '_>) -> Result<(), PongError> {
        let _req: Pong = self.client.parse(self, parser)?;
        Ok(())
    }
}

global_base!(XdgWmBaseGlobal, XdgWmBase, XdgWmBaseError);

impl Global for XdgWmBaseGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        3
    }
}

simple_add_global!(XdgWmBaseGlobal);

object_base! {
    XdgWmBase, XdgWmBaseError;

    DESTROY => destroy,
    CREATE_POSITIONER => create_positioner,
    GET_XDG_SURFACE => get_xdg_surface,
    PONG => pong,
}

dedicated_add_obj!(XdgWmBase, XdgWmBaseId, xdg_wm_bases);

impl Object for XdgWmBase {
    fn num_requests(&self) -> u32 {
        PONG + 1
    }

    fn break_loops(&self) {
        self.surfaces.clear();
    }
}
