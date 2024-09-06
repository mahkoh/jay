use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::{
            AttachedSubsurfaceState, CommitAction, PendingState, StackElement, SurfaceExt,
            SurfaceRole, WlSurface, WlSurfaceError, WlSurfaceId,
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        utils::{
            clonecell::CloneCell,
            linkedlist::{LinkedNode, NodeRef},
            numcell::NumCell,
        },
        wire::{wl_subsurface::*, WlSubsurfaceId},
    },
    std::{
        cell::{Cell, RefCell, RefMut},
        collections::hash_map::OccupiedEntry,
        mem,
        rc::Rc,
    },
    thiserror::Error,
};

#[expect(dead_code)]
const BAD_SURFACE: u32 = 0;

const MAX_SUBSURFACE_DEPTH: u32 = 100;

linear_ids!(SubsurfaceIds, SubsurfaceId, u64);

pub struct WlSubsurface {
    id: WlSubsurfaceId,
    unique_id: SubsurfaceId,
    pub surface: Rc<WlSurface>,
    pub(super) parent: Rc<WlSurface>,
    pub position: Cell<Rect>,
    sync_requested: Cell<bool>,
    sync_ancestor: Cell<bool>,
    node: RefCell<Option<LinkedNode<StackElement>>>,
    latest_node: CloneCell<Option<NodeRef<StackElement>>>,
    depth: NumCell<u32>,
    pub tracker: Tracker<Self>,
    had_buffer: Cell<bool>,
    version: Version,
}

#[derive(Default)]
pub struct PendingSubsurfaceData {
    pub(super) state: Option<Box<PendingState>>,
    node: Option<LinkedNode<StackElement>>,
    position: Option<(i32, i32)>,
}

impl PendingSubsurfaceData {
    pub fn merge(&mut self, next: &mut Self, client: &Rc<Client>) {
        if let Some(mut new) = next.state.take() {
            match &mut self.state {
                Some(old) => old.merge(&mut new, client),
                _ => self.state = Some(new),
            }
        }

        macro_rules! opt {
            ($name:ident) => {
                if let Some(n) = next.$name.take() {
                    self.$name = Some(n);
                }
            };
        }
        opt!(node);
        opt!(position);
    }
}

fn update_children_attach(surface: &WlSubsurface) -> Result<(), WlSubsurfaceError> {
    if surface.depth.get() > MAX_SUBSURFACE_DEPTH {
        return Err(WlSubsurfaceError::MaxDepthExceeded);
    }
    let children = surface.surface.children.borrow();
    if let Some(children) = &*children {
        for child in children.subsurfaces.values() {
            child.sync_ancestor.set(surface.sync());
            child.depth.set(surface.depth.get() + 1);
            update_children_attach(child)?;
        }
    }
    Ok(())
}

impl WlSubsurface {
    pub fn new(
        id: WlSubsurfaceId,
        surface: &Rc<WlSurface>,
        parent: &Rc<WlSurface>,
        version: Version,
    ) -> Self {
        Self {
            id,
            unique_id: surface.client.state.subsurface_ids.next(),
            surface: surface.clone(),
            parent: parent.clone(),
            position: Cell::new(Default::default()),
            sync_requested: Cell::new(true),
            sync_ancestor: Cell::new(false),
            node: RefCell::new(None),
            latest_node: Default::default(),
            depth: NumCell::new(1),
            tracker: Default::default(),
            had_buffer: Cell::new(false),
            version,
        }
    }

    fn pending<'a>(self: &'a Rc<Self>) -> RefMut<'a, PendingSubsurfaceData> {
        RefMut::map(self.parent.pending.borrow_mut(), |m| {
            &mut m
                .subsurfaces
                .entry(self.unique_id)
                .or_insert_with(|| AttachedSubsurfaceState {
                    subsurface: self.clone(),
                    pending: Default::default(),
                })
                .pending
        })
    }

    pub fn apply_state(&self, pending: &mut PendingSubsurfaceData) -> Result<(), WlSurfaceError> {
        if let Some(state) = &mut pending.state.take() {
            self.surface.apply_state(state)?;
        }
        if let Some(v) = pending.node.take() {
            v.pending.set(false);
            self.node.borrow_mut().replace(v);
        }
        if let Some((x, y)) = pending.position.take() {
            self.position
                .set(self.surface.buffer_abs_pos.get().at_point(x, y));
            let (parent_x, parent_y) = self.parent.buffer_abs_pos.get().position();
            self.surface
                .set_absolute_position(parent_x + x, parent_y + y);
            self.parent.need_extents_update.set(true);
        }
        Ok(())
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
        if let Some(ss) = self.parent.ext.get().into_subsurface() {
            self.sync_ancestor.set(ss.sync());
            self.depth.set(ss.depth.get() + 1);
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
        self.latest_node.set(Some(node.to_ref()));
        self.pending().node = Some(node);
        self.surface.set_toplevel(self.parent.toplevel.get());
        self.surface.ext.set(self.clone());
        update_children_attach(self)?;
        let (x, y) = self.parent.buffer_abs_pos.get().position();
        self.surface.set_absolute_position(x, y);
        Ok(())
    }

    fn place(self: &Rc<Self>, sibling: WlSurfaceId, above: bool) -> Result<(), WlSubsurfaceError> {
        if sibling == self.surface.id {
            return Err(WlSubsurfaceError::AboveSelf(sibling));
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
                    _ => return Err(WlSubsurfaceError::NotASibling(sibling, self.surface.id)),
                };
                let sibling_node = match sibling.latest_node.get() {
                    Some(n) => n,
                    _ => return Ok(()),
                };
                match above {
                    true => sibling_node.append(element),
                    _ => sibling_node.prepend(element),
                }
            };
            self.latest_node.set(Some(node.to_ref()));
            self.pending().node.replace(node);
        }
        Ok(())
    }

    pub fn sync(&self) -> bool {
        self.sync_requested.get() || self.sync_ancestor.get()
    }

    fn update_sync(&self, sync: bool) -> Result<(), WlSurfaceError> {
        let was_sync = self.sync();
        self.sync_requested.set(sync);
        let is_sync = self.sync();
        if was_sync != is_sync {
            self.handle_sync_change(is_sync)?;
        }
        Ok(())
    }

    fn handle_sync_change(&self, is_sync: bool) -> Result<(), WlSurfaceError> {
        if !is_sync {
            self.on_desync()?;
        }
        let children = self.surface.children.borrow();
        if let Some(children) = &*children {
            for child in children.subsurfaces.values() {
                let was_sync = child.sync();
                child.sync_ancestor.set(is_sync);
                let is_sync = child.sync();
                if was_sync != is_sync {
                    child.handle_sync_change(is_sync)?;
                }
            }
        }
        Ok(())
    }

    fn on_desync(&self) -> Result<(), WlSurfaceError> {
        let committed = &mut *self.parent.pending.borrow_mut();
        let committed = committed.subsurfaces.get_mut(&self.unique_id);
        if let Some(ps) = committed {
            if let Some(mut state) = ps.pending.state.take() {
                self.surface.apply_state(&mut state)?;
            }
        }
        Ok(())
    }
}

impl WlSubsurfaceRequestHandler for WlSubsurface {
    type Error = WlSubsurfaceError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.unset_ext();
        self.parent.consume_pending_child(self.unique_id, |oe| {
            let oe = oe.remove();
            if let Some(mut state) = oe.pending.state {
                self.surface.apply_state(&mut state)?;
            }
            Ok(())
        })?;
        *self.node.borrow_mut() = None;
        self.latest_node.take();
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
        self.surface.destroy_node();
        Ok(())
    }

    fn set_position(&self, req: SetPosition, slf: &Rc<Self>) -> Result<(), Self::Error> {
        slf.pending().position = Some((req.x, req.y));
        Ok(())
    }

    fn place_above(&self, req: PlaceAbove, slf: &Rc<Self>) -> Result<(), Self::Error> {
        slf.place(req.sibling, true)?;
        Ok(())
    }

    fn place_below(&self, req: PlaceBelow, slf: &Rc<Self>) -> Result<(), Self::Error> {
        slf.place(req.sibling, false)?;
        Ok(())
    }

    fn set_sync(&self, _req: SetSync, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.update_sync(true)?;
        Ok(())
    }

    fn set_desync(&self, _req: SetDesync, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.update_sync(false)?;
        Ok(())
    }
}

object_base! {
    self = WlSubsurface;
    version = self.version;
}

impl Object for WlSubsurface {
    fn break_loops(&self) {
        *self.node.borrow_mut() = None;
        self.latest_node.take();
    }
}

simple_add_obj!(WlSubsurface);

impl SurfaceExt for WlSubsurface {
    fn commit_requested(self: Rc<Self>, pending: &mut Box<PendingState>) -> CommitAction {
        if self.sync() {
            let mut parent_pending = self.pending();
            match &mut parent_pending.state {
                None => parent_pending.state = Some(mem::take(&mut *pending)),
                Some(state) => state.merge(pending, &self.surface.client),
            }
            return CommitAction::AbortCommit;
        }
        CommitAction::ContinueCommit
    }

    fn after_apply_commit(self: Rc<Self>) {
        let has_buffer = self.surface.buffer.is_some();
        if self.had_buffer.replace(has_buffer) != has_buffer {
            if has_buffer {
                if self.parent.visible.get() {
                    let (x, y) = self.surface.buffer_abs_pos.get().position();
                    let extents = self.surface.extents.get();
                    self.surface.client.state.damage(extents.move_(x, y));
                    self.surface.set_visible(true);
                }
            } else {
                self.surface.destroy_node();
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

    fn consume_pending_child(
        &self,
        surface: &WlSurface,
        child: SubsurfaceId,
        consume: &mut dyn FnMut(
            OccupiedEntry<SubsurfaceId, AttachedSubsurfaceState>,
        ) -> Result<(), WlSurfaceError>,
    ) -> Result<(), WlSurfaceError> {
        self.parent
            .consume_pending_child(self.unique_id, |mut oe| {
                let oe = oe.get_mut();
                match &mut oe.pending.state {
                    Some(state) => state.consume_child(child, &mut *consume),
                    _ => Ok(()),
                }
            })?;
        surface.pending.borrow_mut().consume_child(child, consume)
    }
}

#[derive(Debug, Error)]
pub enum WlSubsurfaceError {
    #[error("Surface {0} already has an attached `wl_subsurface`")]
    AlreadyAttached(WlSurfaceId),
    #[error("Surface {0} cannot be made its own parent")]
    OwnParent(WlSurfaceId),
    #[error("Surface {0} cannot be made a subsurface of {1} because it's an ancestor of {1}")]
    Ancestor(WlSurfaceId, WlSurfaceId),
    #[error("Subsurfaces cannot be nested deeper than 100 levels")]
    MaxDepthExceeded,
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Cannot place {0} above/below itself")]
    AboveSelf(WlSurfaceId),
    #[error("{0} is not a sibling of {1}")]
    NotASibling(WlSurfaceId, WlSurfaceId),
}
efrom!(WlSubsurfaceError, WlSurfaceError);
efrom!(WlSubsurfaceError, ClientError);
