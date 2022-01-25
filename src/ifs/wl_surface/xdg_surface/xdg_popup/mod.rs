mod types;

use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceExt};
use crate::object::{Interface, Object, ObjectId};
use crate::tree::{Node, NodeId};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const GRAB: u32 = 1;
const REPOSITION: u32 = 2;

const CONFIGURE: u32 = 0;
const POPUP_DONE: u32 = 1;
const REPOSITIONED: u32 = 2;

#[allow(dead_code)]
const INVALID_GRAB: u32 = 1;

tree_id!(PopupId);
id!(XdgPopupId);

pub struct XdgPopup {
    id: XdgPopupId,
    node_id: PopupId,
    pub(in super::super) surface: Rc<XdgSurface>,
    pub(super) parent: CloneCell<Option<Rc<XdgSurface>>>,
}

impl XdgPopup {
    pub fn new(id: XdgPopupId, surface: &Rc<XdgSurface>, parent: Option<&Rc<XdgSurface>>) -> Self {
        Self {
            id,
            node_id: surface.surface.client.state.node_ids.next(),
            surface: surface.clone(),
            parent: CloneCell::new(parent.cloned()),
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.surface.client.parse(self, parser)?;
        {
            if let Some(parent) = self.parent.take() {
                parent.popups.remove(&self.id);
            }
        }
        Ok(())
    }

    fn grab(&self, parser: MsgParser<'_, '_>) -> Result<(), GrabError> {
        let _req: Grab = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn reposition(&self, parser: MsgParser<'_, '_>) -> Result<(), RepositionError> {
        let _req: Reposition = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgPopupError> {
        match request {
            DESTROY => self.destroy(parser)?,
            GRAB => self.grab(parser)?,
            REPOSITION => self.reposition(parser)?,
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

impl Node for XdgPopup {
    fn id(&self) -> NodeId {
        self.node_id.into()
    }
}

impl XdgSurfaceExt for XdgPopup {}
