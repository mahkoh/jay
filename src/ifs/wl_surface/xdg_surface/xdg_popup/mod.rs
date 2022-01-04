mod types;

use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::ifs::wl_surface::{RoleData, XdgSurfaceRoleData};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const GRAB: u32 = 1;
const REPOSITION: u32 = 2;

const CONFIGURE: u32 = 0;
const POPUP_DONE: u32 = 1;
const REPOSITIONED: u32 = 2;

const INVALID_GRAB: u32 = 1;

id!(XdgPopupId);

pub struct XdgPopup {
    id: XdgPopupId,
    pub(in super::super) surface: Rc<XdgSurface>,
    version: u32,
}

impl XdgPopup {
    pub fn new(id: XdgPopupId, surface: &Rc<XdgSurface>, version: u32) -> Self {
        Self {
            id,
            surface: surface.clone(),
            version,
        }
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.surface.client.parse(self, parser)?;
        {
            let mut rd = self.surface.surface.role_data.borrow_mut();
            if let RoleData::XdgSurface(xdg) = &mut *rd {
                if let XdgSurfaceRoleData::Popup(p) = &xdg.role_data {
                    if let Some(p) = &p.parent {
                        let mut rd = p.surface.role_data.borrow_mut();
                        if let RoleData::XdgSurface(xdg) = &mut *rd {
                            xdg.popups.remove(&self.surface.surface.id);
                        }
                    }
                }
                xdg.role_data = XdgSurfaceRoleData::None;
            }
        }
        Ok(())
    }

    async fn grab(&self, parser: MsgParser<'_, '_>) -> Result<(), GrabError> {
        let _req: Grab = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn reposition(&self, parser: MsgParser<'_, '_>) -> Result<(), RepositionError> {
        let _req: Reposition = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgPopupError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            GRAB => self.grab(parser).await?,
            REPOSITION => self.reposition(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(XdgPopup);

impl Object for XdgPopup {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::XdgPopup
    }

    fn num_requests(&self) -> u32 {
        REPOSITION + 1
    }
}
