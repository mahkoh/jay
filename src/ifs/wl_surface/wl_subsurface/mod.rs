mod types;

use crate::client::AddObj;
use crate::ifs::wl_surface::{
    RoleData, StackElement, SubsurfaceData, SurfaceRole, WlSurface, WlSurfaceId,
};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::cell::Cell;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const SET_POSITION: u32 = 1;
const PLACE_ABOVE: u32 = 2;
const PLACE_BELOW: u32 = 3;
const SET_SYNC: u32 = 4;
const SET_DESYNC: u32 = 5;

const BAD_SURFACE: u32 = 0;

const MAX_SUBSURFACE_DEPTH: u32 = 100;

id!(WlSubsurfaceId);

pub struct WlSubsurface {
    id: WlSubsurfaceId,
    surface: Rc<WlSurface>,
    pub(super) parent: Rc<WlSurface>,
}

fn update_children_sync(surface: &Rc<WlSurface>, sync: bool) -> Result<(), WlSubsurfaceError> {
    let children = surface.children.borrow();
    if let Some(children) = &*children {
        for child in children.subsurfaces.values() {
            let mut data = child.role_data.borrow_mut();
            if let RoleData::Subsurface(data) = &mut *data {
                let was_sync = data.sync_ancestor || data.sync_requested;
                data.sync_ancestor = sync;
                let is_sync = data.sync_ancestor || data.sync_requested;
                if was_sync != is_sync {
                    update_children_sync(child, sync);
                }
            }
        }
    }
    Ok(())
}

fn update_children_attach(
    surface: &Rc<WlSurface>,
    sync: bool,
    depth: u32,
) -> Result<(), WlSubsurfaceError> {
    let children = surface.children.borrow();
    if let Some(children) = &*children {
        for child in children.subsurfaces.values() {
            let mut data = child.role_data.borrow_mut();
            if let RoleData::Subsurface(data) = &mut *data {
                data.depth = depth + 1;
                if data.depth > MAX_SUBSURFACE_DEPTH {
                    return Err(WlSubsurfaceError::MaxDepthExceeded);
                }
                data.sync_ancestor = sync;
                let sync = data.sync_ancestor || data.sync_requested;
                update_children_attach(child, sync, depth + 1);
            }
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
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WlSubsurfaceError> {
        if self.surface.id == self.parent.id {
            return Err(WlSubsurfaceError::OwnParent(self.surface.id));
        }
        let old_ty = self.surface.role.get();
        if !matches!(old_ty, SurfaceRole::None | SurfaceRole::Subsurface) {
            return Err(WlSubsurfaceError::IncompatibleType(self.surface.id, old_ty));
        }
        self.surface.role.set(SurfaceRole::Subsurface);
        let mut data = self.surface.role_data.borrow_mut();
        if matches!(*data, RoleData::Subsurface(_)) {
            return Err(WlSubsurfaceError::AlreadyAttached(self.surface.id));
        }
        if self.surface.id == self.parent.get_root().id {
            return Err(WlSubsurfaceError::Ancestor(self.surface.id, self.parent.id));
        }
        let mut sync_ancestor = false;
        let mut depth = 1;
        {
            let data = self.parent.role_data.borrow();
            if let RoleData::Subsurface(data) = &*data {
                sync_ancestor = data.sync_requested || data.sync_ancestor;
                depth = data.depth + 1;
                if depth >= MAX_SUBSURFACE_DEPTH {
                    return Err(WlSubsurfaceError::MaxDepthExceeded);
                }
            }
        }
        let node = {
            let mut data = self.parent.children.borrow_mut();
            let data = data.get_or_insert_with(|| Default::default());
            data.subsurfaces
                .insert(self.surface.id, self.surface.clone());
            data.above.add_first(StackElement {
                pending: Cell::new(true),
                surface: self.surface.clone(),
            })
        };
        *data = RoleData::Subsurface(Box::new(SubsurfaceData {
            subsurface: self.clone(),
            x: 0,
            y: 0,
            sync_requested: false,
            sync_ancestor,
            depth,
            node,
            pending: Default::default(),
        }));
        update_children_attach(&self.surface, sync_ancestor, depth)?;

        Ok(())
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
        *self.surface.role_data.borrow_mut() = RoleData::None;
        {
            let mut children = self.parent.children.borrow_mut();
            if let Some(children) = &mut *children {
                children.subsurfaces.remove(&self.surface.id);
            }
        }
        self.surface.client.remove_obj(self).await?;
        self.parent.calculate_extents();
        Ok(())
    }

    async fn set_position(&self, parser: MsgParser<'_, '_>) -> Result<(), SetPositionError> {
        let req: SetPosition = self.surface.client.parse(self, parser)?;
        let mut data = self.surface.role_data.borrow_mut();
        if let RoleData::Subsurface(data) = &mut *data {
            data.pending.position = Some((req.x, req.y));
        }
        Ok(())
    }

    fn place(&self, sibling: WlSurfaceId, above: bool) -> Result<(), PlacementError> {
        if sibling == self.surface.id {
            return Err(PlacementError::AboveSelf(sibling));
        }
        let mut data = self.surface.role_data.borrow_mut();
        let pdata = self.parent.children.borrow();
        if let (RoleData::Subsurface(data), Some(pdata)) = (&mut *data, &*pdata) {
            let element = StackElement {
                pending: Cell::new(true),
                surface: self.surface.clone(),
            };
            if sibling == self.parent.id {
                let node = match above {
                    true => pdata.above.add_first(element),
                    _ => pdata.below.add_last(element),
                };
                data.pending.node = Some(node);
            } else {
                let sibling = match pdata.subsurfaces.get(&sibling) {
                    Some(s) => s,
                    _ => return Err(PlacementError::NotASibling(sibling, self.surface.id)),
                };
                let sdata = sibling.role_data.borrow();
                if let RoleData::Subsurface(p) = &*sdata {
                    let node = match &p.pending.node {
                        Some(n) => n,
                        _ => &p.node,
                    };
                    let node = match above {
                        true => node.append(element),
                        _ => node.prepend(element),
                    };
                    data.pending.node = Some(node);
                }
            }
        }
        Ok(())
    }

    async fn place_above(&self, parser: MsgParser<'_, '_>) -> Result<(), PlaceAboveError> {
        let req: PlaceAbove = self.surface.client.parse(self, parser)?;
        self.place(req.sibling, true)?;
        Ok(())
    }

    async fn place_below(&self, parser: MsgParser<'_, '_>) -> Result<(), PlaceBelowError> {
        let req: PlaceBelow = self.surface.client.parse(self, parser)?;
        self.place(req.sibling, false)?;
        Ok(())
    }

    fn update_sync(&self, sync: bool) {
        let mut data = self.surface.role_data.borrow_mut();
        if let RoleData::Subsurface(data) = &mut *data {
            let was_sync = data.sync_requested || data.sync_ancestor;
            data.sync_requested = sync;
            let is_sync = data.sync_requested || data.sync_ancestor;
            if was_sync != is_sync {
                update_children_sync(&self.surface, is_sync);
            }
        }
    }

    async fn set_sync(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSyncError> {
        let _req: SetSync = self.surface.client.parse(self, parser)?;
        self.update_sync(true);
        Ok(())
    }

    async fn set_desync(&self, parser: MsgParser<'_, '_>) -> Result<(), SetDesyncError> {
        let _req: SetDesync = self.surface.client.parse(self, parser)?;
        self.update_sync(false);
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSubsurfaceError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            SET_POSITION => self.set_position(parser).await?,
            PLACE_ABOVE => self.place_above(parser).await?,
            PLACE_BELOW => self.place_below(parser).await?,
            SET_SYNC => self.set_sync(parser).await?,
            SET_DESYNC => self.set_desync(parser).await?,
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
