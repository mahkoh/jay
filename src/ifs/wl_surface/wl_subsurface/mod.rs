mod types;

use crate::ifs::wl_surface::{
    CommitAction, CommitContext, StackElement, SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError,
    WlSurfaceId,
};
use crate::object::{Interface, Object, ObjectId};
use crate::rect::Rect;
use crate::utils::buffd::MsgParser;
use crate::utils::linkedlist::LinkedNode;
use crate::NumCell;
use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const SET_POSITION: u32 = 1;
const PLACE_ABOVE: u32 = 2;
const PLACE_BELOW: u32 = 3;
const SET_SYNC: u32 = 4;
const SET_DESYNC: u32 = 5;

#[allow(dead_code)]
const BAD_SURFACE: u32 = 0;

const MAX_SUBSURFACE_DEPTH: u32 = 100;

id!(WlSubsurfaceId);

pub struct WlSubsurface {
    id: WlSubsurfaceId,
    pub surface: Rc<WlSurface>,
    pub(super) parent: Rc<WlSurface>,
    pub position: Cell<Rect>,
    sync_requested: Cell<bool>,
    sync_ancestor: Cell<bool>,
    node: RefCell<Option<LinkedNode<StackElement>>>,
    depth: NumCell<u32>,
    pending: PendingSubsurfaceData,
}

#[derive(Default)]
struct PendingSubsurfaceData {
    node: RefCell<Option<LinkedNode<StackElement>>>,
    position: Cell<Option<(i32, i32)>>,
}

fn update_children_sync(surface: &WlSubsurface, sync: bool) {
    let children = surface.surface.children.borrow();
    if let Some(children) = &*children {
        for child in children.subsurfaces.values() {
            let was_sync = child.sync();
            child.sync_ancestor.set(sync);
            let is_sync = child.sync();
            if was_sync != is_sync {
                update_children_sync(child, sync);
            }
        }
    }
}

fn update_children_attach(
    surface: &WlSubsurface,
    mut sync: bool,
    depth: u32,
) -> Result<(), WlSubsurfaceError> {
    let children = surface.surface.children.borrow();
    if let Some(children) = &*children {
        for child in children.subsurfaces.values() {
            child.depth.set(depth + 1);
            if depth + 1 > MAX_SUBSURFACE_DEPTH {
                return Err(WlSubsurfaceError::MaxDepthExceeded);
            }
            child.sync_ancestor.set(sync);
            sync |= child.sync_requested.get();
            update_children_attach(child, sync, depth + 1)?;
        }
    }
    Ok(())
}

impl WlSubsurface {
    pub fn new(id: WlSubsurfaceId, surface: &Rc<WlSurface>, parent: &Rc<WlSurface>) -> Self {
        Self {
            id,
            surface: surface.clone(),
            parent: parent.clone(),
            position: Cell::new(Default::default()),
            sync_requested: Cell::new(false),
            sync_ancestor: Cell::new(false),
            node: RefCell::new(None),
            depth: NumCell::new(0),
            pending: Default::default(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WlSubsurfaceError> {
        if self.surface.id == self.parent.id {
            return Err(WlSubsurfaceError::OwnParent(self.surface.id));
        }
        self.surface.set_role(SurfaceRole::Subsurface)?;
        if self.surface.ext.get().is_some() {
            return Err(WlSubsurfaceError::AlreadyAttached(self.surface.id));
        }
        if self.surface.id == self.parent.get_root().id {
            return Err(WlSubsurfaceError::Ancestor(self.surface.id, self.parent.id));
        }
        let mut sync_ancestor = false;
        let mut depth = 1;
        {
            if let Some(ss) = self.parent.ext.get().into_subsurface() {
                sync_ancestor = ss.sync();
                depth = ss.depth.get() + 1;
                if depth >= MAX_SUBSURFACE_DEPTH {
                    return Err(WlSubsurfaceError::MaxDepthExceeded);
                }
            }
        }
        let node = {
            let mut data = self.parent.children.borrow_mut();
            let data = data.get_or_insert_with(Default::default);
            data.subsurfaces.insert(self.surface.id, self.clone());
            data.above.add_first(StackElement {
                pending: Cell::new(true),
                sub_surface: self.clone(),
            })
        };
        *self.pending.node.borrow_mut() = Some(node);
        self.sync_ancestor.set(sync_ancestor);
        self.depth.set(depth);
        self.surface.ext.set(self.clone());
        update_children_attach(&self, sync_ancestor, depth)?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
        self.surface.unset_ext();
        *self.node.borrow_mut() = None;
        {
            let mut children = self.parent.children.borrow_mut();
            if let Some(children) = &mut *children {
                children.subsurfaces.remove(&self.surface.id);
            }
        }
        if !self.surface.extents.get().is_empty() {
            let mut parent_opt = Some(self.parent.clone());
            while let Some(parent) = parent_opt.take() {
                if !parent.need_extents_update.get() {
                    break;
                }
                parent.calculate_extents();
                parent_opt = parent.ext.get().subsurface_parent();
            }
        }
        self.surface.client.remove_obj(self)?;
        Ok(())
    }

    fn set_position(&self, parser: MsgParser<'_, '_>) -> Result<(), SetPositionError> {
        let req: SetPosition = self.surface.client.parse(self, parser)?;
        self.pending.position.set(Some((req.x, req.y)));
        Ok(())
    }

    fn place(self: &Rc<Self>, sibling: WlSurfaceId, above: bool) -> Result<(), PlacementError> {
        if sibling == self.surface.id {
            return Err(PlacementError::AboveSelf(sibling));
        }
        let pdata = self.parent.children.borrow();
        if let Some(pdata) = &*pdata {
            let element = StackElement {
                pending: Cell::new(true),
                sub_surface: self.clone(),
            };
            let node = if sibling == self.parent.id {
                match above {
                    true => pdata.above.add_first(element),
                    _ => pdata.below.add_last(element),
                }
            } else {
                let sibling = match pdata.subsurfaces.get(&sibling) {
                    Some(s) => s,
                    _ => return Err(PlacementError::NotASibling(sibling, self.surface.id)),
                };
                let node = match sibling.pending.node.borrow().deref() {
                    Some(n) => n.to_ref(),
                    _ => match sibling.node.borrow().deref() {
                        Some(n) => n.to_ref(),
                        _ => return Ok(()),
                    },
                };
                match above {
                    true => node.append(element),
                    _ => node.prepend(element),
                }
            };
            self.pending.node.borrow_mut().replace(node);
        }
        Ok(())
    }

    fn place_above(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), PlaceAboveError> {
        let req: PlaceAbove = self.surface.client.parse(self.deref(), parser)?;
        self.place(req.sibling, true)?;
        Ok(())
    }

    fn place_below(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), PlaceBelowError> {
        let req: PlaceBelow = self.surface.client.parse(self.deref(), parser)?;
        self.place(req.sibling, false)?;
        Ok(())
    }

    pub fn sync(&self) -> bool {
        self.sync_requested.get() || self.sync_ancestor.get()
    }

    fn update_sync(&self, sync: bool) {
        let was_sync = self.sync();
        self.sync_requested.set(sync);
        let is_sync = self.sync();
        if was_sync != is_sync {
            update_children_sync(self, is_sync);
        }
    }

    fn set_sync(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSyncError> {
        let _req: SetSync = self.surface.client.parse(self, parser)?;
        self.update_sync(true);
        Ok(())
    }

    fn set_desync(&self, parser: MsgParser<'_, '_>) -> Result<(), SetDesyncError> {
        let _req: SetDesync = self.surface.client.parse(self, parser)?;
        self.update_sync(false);
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSubsurfaceError> {
        match request {
            DESTROY => self.destroy(parser)?,
            SET_POSITION => self.set_position(parser)?,
            PLACE_ABOVE => self.place_above(parser)?,
            PLACE_BELOW => self.place_below(parser)?,
            SET_SYNC => self.set_sync(parser)?,
            SET_DESYNC => self.set_desync(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlSubsurface);

impl Object for WlSubsurface {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlSubsurface
    }

    fn num_requests(&self) -> u32 {
        SET_DESYNC + 1
    }
}

impl SurfaceExt for WlSubsurface {
    fn pre_commit(self: Rc<Self>, ctx: CommitContext) -> Result<CommitAction, WlSurfaceError> {
        if ctx == CommitContext::RootCommit && self.sync() {
            log::info!("Aborting commit due to sync");
            return Ok(CommitAction::AbortCommit);
        }
        Ok(CommitAction::ContinueCommit)
    }

    fn post_commit(&self) {
        if let Some(v) = self.pending.node.take() {
            log::info!("post commit");
            v.pending.set(false);
            self.node.borrow_mut().replace(v);
        }
        if let Some((x, y)) = self.pending.position.take() {
            if let Some(buffer) = self.surface.buffer.get() {
                self.position.set(buffer.rect.move_(x, y));
                self.parent.need_extents_update.set(true);
            } else {
                self.position.set(Rect::new_empty(x, y));
            }
        }
    }

    fn subsurface_parent(&self) -> Option<Rc<WlSurface>> {
        Some(self.parent.clone())
    }

    fn extents_changed(&self) {
        self.parent.need_extents_update.set(true);
    }

    fn into_subsurface(self: Rc<Self>) -> Option<Rc<WlSubsurface>> {
        Some(self)
    }
}
