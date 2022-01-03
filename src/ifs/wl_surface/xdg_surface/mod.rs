mod types;
pub mod xdg_popup;
pub mod xdg_toplevel;

use crate::ifs::wl_surface::{RoleData, SurfaceRole, WlSurface, XdgSurfaceData};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;
use crate::client::AddObj;
use crate::ifs::xdg_wm_base::XdgWmBaseObj;

const DESTROY: u32 = 0;
const GET_TOPLEVEL: u32 = 1;
const GET_POPUP: u32 = 2;
const SET_WINDOW_GEOMETRY: u32 = 3;
const ACK_CONFIGURE: u32 = 4;

const CONFIGURE: u32 = 0;

const NOT_CONSTRUCTED: u32 = 1;
const ALREADY_CONSTRUCTED: u32 = 2;
const UNCONFIGURED_BUFFER: u32 = 3;

pub struct XdgSurface {
    id: ObjectId,
    wm_base: Rc<XdgWmBaseObj>,
    surface: Rc<WlSurface>,
    version: u32,
}

impl XdgSurface {
    pub fn new(wm_base: &Rc<XdgWmBaseObj>, id: ObjectId, surface: &Rc<WlSurface>, version: u32) -> Self {
        Self {
            id,
            wm_base: wm_base.clone(),
            surface: surface.clone(),
            version,
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), XdgSurfaceError> {
        let old_role = self.surface.role.get();
        if !matches!(old_role, SurfaceRole::None | SurfaceRole::XdgSurface) {
            return Err(XdgSurfaceError::IncompatibleRole(self.surface.id, old_role));
        }
        let mut data = self.surface.role_data.borrow_mut();
        if data.is_some() {
            return Err(XdgSurfaceError::AlreadyAttached(self.surface.id));
        }
        *data = RoleData::XdgSurface(Box::new(XdgSurfaceData {
            xdg_surface: self.clone(),
        }));
        Ok(())
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
        *self.surface.role_data.borrow_mut() = RoleData::None;
        self.wm_base.surfaces.remove(&self.id);
        self.surface.client.remove_obj(self).await?;
        Ok(())
    }

    async fn get_toplevel(&self, parser: MsgParser<'_, '_>) -> Result<(), GetToplevelError> {
        let _req: GetToplevel = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn get_popup(&self, parser: MsgParser<'_, '_>) -> Result<(), GetPopupError> {
        let _req: GetPopup = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_window_geometry(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), SetWindowGeometryError> {
        let _req: SetWindowGeometry = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn ack_configure(&self, parser: MsgParser<'_, '_>) -> Result<(), AckConfigureError> {
        let _req: AckConfigure = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgSurfaceError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            GET_TOPLEVEL => self.get_toplevel(parser).await?,
            GET_POPUP => self.get_popup(parser).await?,
            SET_WINDOW_GEOMETRY => self.set_window_geometry(parser).await?,
            ACK_CONFIGURE => self.ack_configure(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(XdgSurface);

impl Object for XdgSurface {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::XdgSurface
    }

    fn num_requests(&self) -> u32 {
        ACK_CONFIGURE + 1
    }
}
