use {
    crate::{
        client::ClientError,
        ifs::wl_surface::{
            CommitAction, CommittedSubsurface, PendingState, StackElement, SurfaceExt, SurfaceRole,
            WlSurface, WlSurfaceError, WlSurfaceId,
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
            linkedlist::{LinkedNode, NodeRef},
            numcell::NumCell,
            option_ext::OptionExt,
        },
        wire::{wl_subsurface::*, WlSubsurfaceId},
    },
    std::{
        cell::{Cell, RefCell, RefMut},
        collections::hash_map::{Entry, OccupiedEntry},
        mem,
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
};

#[allow(dead_code)]
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
}

#[derive(Default)]
pub struct PendingSubsurfaceData {
    node: Option<LinkedNode<StackElement>>,
    position: Option<(i32, i32)>,
}

impl PendingSubsurfaceData {
    pub fn merge(&mut self, next: &mut Self) {
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
            unique_id: surface.client.state.subsurface_ids.next(),
            surface: surface.clone(),
            parent: parent.clone(),
            position: Cell::new(Default::default()),
            sync_requested: Cell::new(true),
            sync_ancestor: Cell::new(false),
            node: RefCell::new(None),
            latest_node: Default::default(),
            depth: NumCell::new(0),
            tracker: Default::default(),
            had_buffer: Cell::new(false),
        }
    }

    fn pending(&self) -> RefMut<Box<PendingSubsurfaceData>> {
        RefMut::map(self.surface.pending.borrow_mut(), |m| {
            m.subsurface.get_or_insert_default_ext()
        })
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
        self.latest_node.set(Some(node.to_ref()));
        self.pending().node = Some(node);
        self.surface.set_toplevel(self.parent.toplevel.get());
        self.sync_ancestor.set(sync_ancestor);
        self.depth.set(depth);
        self.surface.ext.set(self.clone());
        update_children_attach(self, sync_ancestor, depth)?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSubsurfaceError> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
        self.surface.unset_ext();
        self.parent.consume_pending_child(self.unique_id, |oe| {
            self.surface.apply_state(&mut oe.remove().state)
        })?;
        self.surface.pending.borrow_mut().subsurface.take();
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

    fn set_position(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSubsurfaceError> {
        let req: SetPosition = self.surface.client.parse(self, parser)?;
        self.pending().position = Some((req.x, req.y));
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

    fn place_above(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), WlSubsurfaceError> {
        let req: PlaceAbove = self.surface.client.parse(self.deref(), parser)?;
        self.place(req.sibling, true)?;
        Ok(())
    }

    fn place_below(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), WlSubsurfaceError> {
        let req: PlaceBelow = self.surface.client.parse(self.deref(), parser)?;
        self.place(req.sibling, false)?;
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
        let committed = self
            .parent
            .pending
            .borrow_mut()
            .subsurfaces
            .remove(&self.unique_id);
        if let Some(mut ps) = committed {
            self.surface.apply_state(&mut ps.state)?;
        }
        Ok(())
    }

    fn set_sync(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSubsurfaceError> {
        let _req: SetSync = self.surface.client.parse(self, parser)?;
        self.update_sync(true)?;
        Ok(())
    }

    fn set_desync(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSubsurfaceError> {
        let _req: SetDesync = self.surface.client.parse(self, parser)?;
        self.update_sync(false)?;
        Ok(())
    }
}

object_base! {
    self = WlSubsurface;

    DESTROY => destroy,
    SET_POSITION => set_position,
    PLACE_ABOVE => place_above,
    PLACE_BELOW => place_below,
    SET_SYNC => set_sync,
    SET_DESYNC => set_desync,
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
            let mut parent_pending = self.parent.pending.borrow_mut();
            match parent_pending.subsurfaces.entry(self.unique_id) {
                Entry::Occupied(mut o) => {
                    o.get_mut().state.merge(pending, &self.surface.client);
                }
                Entry::Vacant(v) => {
                    v.insert(CommittedSubsurface {
                        subsurface: self.clone(),
                        state: mem::take(&mut *pending),
                    });
                }
            }
            return CommitAction::AbortCommit;
        }
        CommitAction::ContinueCommit
    }

    fn after_apply_commit(self: Rc<Self>, pending: &mut PendingState) {
        if let Some(pending) = &mut pending.subsurface {
            if let Some(v) = pending.node.take() {
                v.pending.set(false);
                self.node.borrow_mut().replace(v);
            }
            if let Some((x, y)) = pending.position.take() {
                self.position
                    .set(self.surface.buffer_abs_pos.get().at_point(x, y));
                self.parent.need_extents_update.set(true);
            }
        }
        let has_buffer = self.surface.buffer.is_some();
        if self.had_buffer.replace(has_buffer) != has_buffer {
            if has_buffer {
                if self.parent.visible.get() {
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
            OccupiedEntry<SubsurfaceId, CommittedSubsurface>,
        ) -> Result<(), WlSurfaceError>,
    ) -> Result<(), WlSurfaceError> {
        self.parent
            .consume_pending_child(self.unique_id, |mut oe| {
                oe.get_mut().state.consume_child(child, &mut *consume)
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
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("Cannot place {0} above/below itself")]
    AboveSelf(WlSurfaceId),
    #[error("{0} is not a sibling of {1}")]
    NotASibling(WlSurfaceId, WlSurfaceId),
}
efrom!(WlSubsurfaceError, WlSurfaceError);
efrom!(WlSubsurfaceError, MsgParserError);
efrom!(WlSubsurfaceError, ClientError);
