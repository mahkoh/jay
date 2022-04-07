use {
    crate::{
        client::ClientError,
        ifs::wl_surface::{
            CommitAction, CommitContext, StackElement, SurfaceExt, SurfaceRole, WlSurface,
            WlSurfaceError, WlSurfaceId,
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        utils::{
            buffd::{MsgParser, MsgParserError},
            linkedlist::LinkedNode,
            numcell::NumCell,
        },
        wire::{wl_subsurface::*, WlSubsurfaceId},
    },
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
};

#[allow(dead_code)]
const BAD_SURFACE: u32 = 0;

const MAX_SUBSURFACE_DEPTH: u32 = 100;

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
    pub tracker: Tracker<Self>,
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
            tracker: Default::default(),
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
        update_children_attach(self, sync_ancestor, depth)?;
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
}

object_base! {
    WlSubsurface, WlSubsurfaceError;

    DESTROY => destroy,
    SET_POSITION => set_position,
    PLACE_ABOVE => place_above,
    PLACE_BELOW => place_below,
    SET_SYNC => set_sync,
    SET_DESYNC => set_desync,
}

impl Object for WlSubsurface {
    fn num_requests(&self) -> u32 {
        SET_DESYNC + 1
    }
}

simple_add_obj!(WlSubsurface);

impl SurfaceExt for WlSubsurface {
    fn pre_commit(self: Rc<Self>, ctx: CommitContext) -> Result<CommitAction, WlSurfaceError> {
        if ctx == CommitContext::RootCommit && self.sync() {
            log::info!("Aborting commit due to sync");
            return Ok(CommitAction::AbortCommit);
        }
        Ok(CommitAction::ContinueCommit)
    }

    fn post_commit(self: Rc<Self>) {
        if let Some(v) = self.pending.node.take() {
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

    fn accepts_kb_focus(&self) -> bool {
        self.parent.accepts_kb_focus()
    }
}

#[derive(Debug, Error)]
pub enum WlSubsurfaceError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `set_position` request")]
    SetPosition(#[from] SetPositionError),
    #[error("Could not process `place_above` request")]
    PlaceAbove(#[from] PlaceAboveError),
    #[error("Could not process `place_below` request")]
    PlaceBelow(#[from] PlaceBelowError),
    #[error("Could not process `set_sync` request")]
    SetSync(#[from] SetSyncError),
    #[error("Could not process `set_desync` request")]
    SetDesync(#[from] SetDesyncError),
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
}
efrom!(WlSubsurfaceError, WlSurfaceError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum SetPositionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetPositionError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum PlaceAboveError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    PlacementError(#[from] PlacementError),
}
efrom!(PlaceAboveError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum PlacementError {
    #[error("Cannot place {0} above/below itself")]
    AboveSelf(WlSurfaceId),
    #[error("{0} is not a sibling of {1}")]
    NotASibling(WlSurfaceId, WlSurfaceId),
}

#[derive(Debug, Error)]
pub enum PlaceBelowError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    PlacementError(#[from] PlacementError),
}
efrom!(PlaceBelowError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SetSyncError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetSyncError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SetDesyncError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetDesyncError, ParseFailed, MsgParserError);
