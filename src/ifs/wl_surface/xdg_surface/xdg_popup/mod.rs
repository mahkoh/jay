mod types;

use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceError, XdgSurfaceExt};
use crate::ifs::xdg_positioner::{XdgPositioned, XdgPositioner};
use crate::object::{Interface, Object, ObjectId};
use crate::rect::Rect;
use crate::tree::{FoundNode, Node, NodeId, StackedNode, WorkspaceNode};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
pub use types::*;
use crate::client::DynEventFormatter;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::wl_surface::SurfaceExt;
use crate::render::Renderer;
use crate::utils::linkedlist::LinkedNode;

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
    pub xdg: Rc<XdgSurface>,
    pub(super) parent: CloneCell<Option<Rc<XdgSurface>>>,
    relative_position: Cell<Rect>,
    display_link: RefCell<Option<LinkedNode<StackedNode>>>,
    workspace_link: RefCell<Option<LinkedNode<StackedNode>>>,
    pos: RefCell<XdgPositioned>,
}

impl XdgPopup {
    pub fn new(
        id: XdgPopupId,
        xdg: &Rc<XdgSurface>,
        parent: Option<&Rc<XdgSurface>>,
        pos: &Rc<XdgPositioner>,
    ) -> Result<Self, XdgPopupError> {
        let pos = pos.value();
        if !pos.is_complete() {
            return Err(XdgPopupError::Incomplete);
        }
        Ok(Self {
            id,
            node_id: xdg.surface.client.state.node_ids.next(),
            xdg: xdg.clone(),
            parent: CloneCell::new(parent.cloned()),
            relative_position: Cell::new(Default::default()),
            display_link: RefCell::new(None),
            workspace_link: RefCell::new(None),
            pos: RefCell::new(pos),
        })
    }

    fn configure(self: &Rc<Self>, x: i32, y: i32, width: i32, height: i32) -> DynEventFormatter {
        Box::new(Configure {
            obj: self.clone(),
            x,
            y,
            width,
            height,
        })
    }

    fn repositioned(self: &Rc<Self>, token: u32) -> DynEventFormatter {
        Box::new(Repositioned {
            obj: self.clone(),
            token,
        })
    }

    fn popup_done(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(PopupDone {
            obj: self.clone(),
        })
    }

    fn update_relative_position(&self, parent: &XdgSurface) -> Result<(), XdgPopupError> {
        let parent = parent.extents.get();
        let positioner = self.pos.borrow();
        if !parent.contains_rect(&positioner.ar) {
            // return Err(XdgPopupError::AnchorRectOutside);
        }
        self.relative_position.set(positioner.get_position());
        Ok(())
    }

    pub fn update_absolute_position(&self) {
        if let Some(parent) = self.parent.get() {
            let rel = self.relative_position.get();
            let parent = parent.absolute_desired_extents.get();
            self.xdg.absolute_desired_extents.set(rel.move_(parent.x1(), parent.y1()));
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.xdg.surface.client.parse(self, parser)?;
        {
            if let Some(parent) = self.parent.take() {
                parent.popups.remove(&self.id);
            }
        }
        self.xdg.ext.set(None);
        self.xdg.surface.client.remove_obj(self)?;
        self.clear();
        *self.display_link.borrow_mut() = None;
        *self.workspace_link.borrow_mut() = None;
        Ok(())
    }

    fn grab(&self, parser: MsgParser<'_, '_>) -> Result<(), GrabError> {
        let _req: Grab = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn reposition(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), RepositionError> {
        let req: Reposition = self.xdg.surface.client.parse(&**self, parser)?;
        *self.pos.borrow_mut() = self.xdg.surface.client.get_xdg_positioner(req.positioner)?.value();
        if let Some(parent) = self.parent.get() {
            self.update_relative_position(&parent)?;
            let rel = self.relative_position.get();
            self.xdg.surface.client.event(self.repositioned(req.token));
            self.xdg.surface.client.event(self.configure(rel.x1(), rel.y1(), rel.width(), rel.height()));
            self.xdg.send_configure();
            let parent = parent.absolute_desired_extents.get();
            self.xdg.absolute_desired_extents.set(rel.move_(parent.x1(), parent.y1()));
        }
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
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

    fn break_loops(&self) {
        self.parent.set(None);
        self.clear();
        *self.display_link.borrow_mut() = None;
        *self.workspace_link.borrow_mut() = None;
    }
}

impl Node for XdgPopup {
    fn id(&self) -> NodeId {
        self.node_id.into()
    }

    fn clear(&self) {
        let _v = self.display_link.borrow_mut().take();
        let _v = self.workspace_link.borrow_mut().take();
    }

    fn find_child_at(&self, x: i32, y: i32) -> Option<FoundNode> {
        self.xdg.find_child_at(x, y)
    }

    fn enter(self: Rc<Self>, seat: &WlSeatGlobal, _x: Fixed, _y: Fixed) {
        seat.enter_popup(&self);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_xdg_surface(&self.xdg, x, y)
    }

    fn get_workspace(self: Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        self.parent.get()?.into_node()?.get_workspace()
    }
}

impl XdgSurfaceExt for XdgPopup {
    fn initial_configure(self: Rc<Self>) -> Result<(), XdgSurfaceError> {
        if let Some(parent) = self.parent.get() {
            self.update_relative_position(&parent)?;
            let rel = self.relative_position.get();
            self.xdg.surface.client.event(self.configure(rel.x1(), rel.y1(), rel.width(), rel.height()));
            let parent = parent.absolute_desired_extents.get();
            self.xdg.absolute_desired_extents.set(rel.move_(parent.x1(), parent.y1()));
        }
        Ok(())
    }

    fn post_commit(self: Rc<Self>) {
        let mut wl = self.workspace_link.borrow_mut();
        let mut dl = self.display_link.borrow_mut();
        let ws = match self.clone().get_workspace() {
            Some(ws) => ws,
            _ => return,
        };
        let surface = &self.xdg.surface;
        let state = &surface.client.state;
        if surface.buffer.get().is_some() {
            if wl.is_none() {
                *wl = Some(ws.stacked.add_last(StackedNode::Popup(self.clone())));
                *dl = Some(state.root.stacked.add_last(StackedNode::Popup(self.clone())));
                state.tree_changed();
            }
        } else {
            if wl.take().is_some() {
                *dl = None;
                state.tree_changed();
                surface.client.event(self.popup_done());
            }
        }
    }

    fn into_node(self: Rc<Self>) -> Option<Rc<dyn Node>> {
        Some(self)
    }

    fn extents_changed(&self) {
        self.xdg.surface.client.state.tree_changed();
    }
}
