mod types;

use crate::client::{AddObj, Client};
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::ifs::xdg_positioner::XdgPositioner;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const CREATE_POSITIONER: u32 = 1;
const GET_XDG_SURFACE: u32 = 2;
const PONG: u32 = 3;

const PING: u32 = 0;

const ROLE: u32 = 0;
const DEFUNCT_SURFACES: u32 = 1;
const NOT_THE_TOPMOST_POPUP: u32 = 2;
const INVALID_POPUP_PARENT: u32 = 3;
const INVALID_SURFACE_STATE: u32 = 4;
const INVALID_POSITIONER: u32 = 5;

pub struct XdgWmBaseGlobal {
    name: GlobalName,
}

pub struct XdgWmBaseObj {
    global: Rc<XdgWmBaseGlobal>,
    id: ObjectId,
    client: Rc<Client>,
    version: u32,
    pub(super) surfaces: CopyHashMap<ObjectId, Rc<XdgSurface>>,
}

impl XdgWmBaseGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    async fn bind_(
        self: Rc<Self>,
        id: ObjectId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), XdgWmBaseError> {
        let obj = Rc::new(XdgWmBaseObj {
            global: self,
            id,
            client: client.clone(),
            version,
            surfaces: Default::default(),
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl XdgWmBaseObj {
    pub fn break_loops(&self) {
        self.surfaces.clear();
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
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
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn create_positioner(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), CreatePositionerError> {
        let req: CreatePositioner = self.client.parse(self, parser)?;
        let pos = Rc::new(XdgPositioner::new(req.id, &self.client, 3));
        self.client.add_client_obj(&pos)?;
        Ok(())
    }

    async fn get_xdg_surface(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetXdgSurfaceError> {
        let req: GetXdgSurface = self.client.parse(&**self, parser)?;
        let surface = self.client.get_surface(req.surface)?;
        let xdg_surface = Rc::new(XdgSurface::new(self, req.id, &surface, 3));
        self.client.add_client_obj(&xdg_surface)?;
        xdg_surface.install()?;
        self.surfaces.set(req.id, xdg_surface);
        Ok(())
    }

    async fn pong(&self, parser: MsgParser<'_, '_>) -> Result<(), PongError> {
        let _req: Pong = self.client.parse(self, parser)?;
        Ok(())
    }

    async fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgWmBaseError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            CREATE_POSITIONER => self.create_positioner(parser).await?,
            GET_XDG_SURFACE => self.get_xdg_surface(parser).await?,
            PONG => self.pong(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

bind!(XdgWmBaseGlobal);

impl Global for XdgWmBaseGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn interface(&self) -> Interface {
        Interface::XdgWmBase
    }

    fn version(&self) -> u32 {
        3
    }

    fn pre_remove(&self) {
        unreachable!()
    }
}

handle_request!(XdgWmBaseObj);

impl Object for XdgWmBaseObj {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::XdgWmBase
    }

    fn num_requests(&self) -> u32 {
        PONG + 1
    }
}
