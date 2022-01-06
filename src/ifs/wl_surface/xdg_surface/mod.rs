mod types;
pub mod xdg_popup;
pub mod xdg_toplevel;

use crate::client::{AddObj, DynEventFormatter};
use crate::ifs::wl_surface::xdg_surface::xdg_popup::XdgPopup;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::{
    RoleData, SurfaceRole, WlSurface, XdgPopupData, XdgSurfaceData, XdgSurfaceRole,
    XdgSurfaceRoleData, XdgToplevelData,
};
use crate::ifs::xdg_wm_base::XdgWmBaseObj;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::ops::DerefMut;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const GET_TOPLEVEL: u32 = 1;
const GET_POPUP: u32 = 2;
const SET_WINDOW_GEOMETRY: u32 = 3;
const ACK_CONFIGURE: u32 = 4;

const CONFIGURE: u32 = 0;

const NOT_CONSTRUCTED: u32 = 1;
const ALREADY_CONSTRUCTED: u32 = 2;
const UNCONFIGURED_BUFFER: u32 = 3;

id!(XdgSurfaceId);

pub struct XdgSurface {
    id: XdgSurfaceId,
    wm_base: Rc<XdgWmBaseObj>,
    pub surface: Rc<WlSurface>,
    version: u32,
}

impl XdgSurface {
    pub fn new(
        wm_base: &Rc<XdgWmBaseObj>,
        id: XdgSurfaceId,
        surface: &Rc<WlSurface>,
        version: u32,
    ) -> Self {
        Self {
            id,
            wm_base: wm_base.clone(),
            surface: surface.clone(),
            version,
        }
    }

    pub fn configure(self: &Rc<Self>, serial: u32) -> DynEventFormatter {
        Box::new(Configure {
            obj: self.clone(),
            serial,
        })
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
            requested_serial: 0,
            acked_serial: None,
            role: XdgSurfaceRole::None,
            role_data: XdgSurfaceRoleData::None,
            popups: Default::default(),
        }));
        Ok(())
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
        {
            let mut data = self.surface.role_data.borrow_mut();
            if let RoleData::XdgSurface(rd) = &*data {
                if rd.role_data.is_some() {
                    return Err(DestroyError::RoleNotYetDestroyed(self.id));
                }
                let children = rd.popups.lock();
                for child in children.values() {
                    let mut data = child.surface.surface.role_data.borrow_mut();
                    if let RoleData::XdgSurface(xdg) = &mut *data {
                        if let XdgSurfaceRoleData::Popup(p) = &mut xdg.role_data {
                            p.parent = None;
                        }
                    }
                }
            }
            *data = RoleData::None;
        }
        self.wm_base.surfaces.remove(&self.id);
        self.surface.client.remove_obj(self).await?;
        Ok(())
    }

    async fn get_toplevel(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), GetToplevelError> {
        let req: GetToplevel = self.surface.client.parse(&**self, parser)?;
        let mut data = self.surface.role_data.borrow_mut();
        if let RoleData::XdgSurface(data) = &mut *data {
            if !data.role.is_compatible(XdgSurfaceRole::Toplevel) {
                return Err(GetToplevelError::IncompatibleRole);
            }
            if data.role_data.is_some() {
                self.surface.client.protocol_error(
                    &**self,
                    ALREADY_CONSTRUCTED,
                    format!(
                        "wl_surface {} already has an assigned xdg_toplevel",
                        self.surface.id
                    ),
                );
                return Err(GetToplevelError::AlreadyConstructed);
            }
            data.role = XdgSurfaceRole::Toplevel;
            let toplevel = Rc::new(XdgToplevel::new(req.id, self, self.version));
            self.surface.client.add_client_obj(&toplevel)?;
            data.role_data = XdgSurfaceRoleData::Toplevel(XdgToplevelData {
                toplevel,
                node: None,
            });
        }
        Ok(())
    }

    async fn get_popup(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetPopupError> {
        let req: GetPopup = self.surface.client.parse(&**self, parser)?;
        let mut data = self.surface.role_data.borrow_mut();
        if let RoleData::XdgSurface(data) = &mut *data {
            let mut parent = None;
            if req.parent.is_some() {
                parent = Some(self.surface.client.get_xdg_surface(req.parent)?);
            }
            if !data.role.is_compatible(XdgSurfaceRole::Popup) {
                return Err(GetPopupError::IncompatibleRole);
            }
            if data.role_data.is_some() {
                self.surface.client.protocol_error(
                    &**self,
                    ALREADY_CONSTRUCTED,
                    format!(
                        "wl_surface {} already has an assigned xdg_popup",
                        self.surface.id
                    ),
                );
                return Err(GetPopupError::AlreadyConstructed);
            }
            data.role = XdgSurfaceRole::Popup;
            let popup = Rc::new(XdgPopup::new(req.id, self, self.version));
            self.surface.client.add_client_obj(&popup)?;
            if let Some(parent) = &parent {
                let mut data = parent.surface.role_data.borrow_mut();
                if let RoleData::XdgSurface(xdg) = &mut *data {
                    xdg.popups.set(self.surface.id, popup.clone());
                }
            }
            data.role_data = XdgSurfaceRoleData::Popup(XdgPopupData { popup, parent });
        }
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
        let req: AckConfigure = self.surface.client.parse(self, parser)?;
        let mut rd = self.surface.role_data.borrow_mut();
        if let RoleData::XdgSurface(xdg) = rd.deref_mut() {
            if xdg.requested_serial == req.serial {
                xdg.acked_serial = Some(xdg.requested_serial);
            }
        }
        Ok(())
    }

    async fn handle_request_(
        self: &Rc<Self>,
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
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::XdgSurface
    }

    fn num_requests(&self) -> u32 {
        ACK_CONFIGURE + 1
    }
}
