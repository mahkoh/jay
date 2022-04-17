pub mod cursor;
pub mod wl_subsurface;
pub mod xdg_surface;
pub mod xwindow;
pub mod zwlr_layer_surface_v1;

use {
    crate::{
        backend::KeyState,
        client::{Client, ClientError, RequestParser},
        fixed::Fixed,
        ifs::{
            wl_buffer::WlBuffer,
            wl_callback::WlCallback,
            wl_seat::{wl_pointer::PendingScroll, Dnd, NodeSeatState, SeatId, WlSeatGlobal},
            wl_surface::{
                cursor::CursorSurface, wl_subsurface::WlSubsurface, xdg_surface::XdgSurfaceError,
                zwlr_layer_surface_v1::ZwlrLayerSurfaceV1Error,
            },
        },
        leaks::Tracker,
        object::Object,
        rect::{Rect, Region},
        render::Renderer,
        tree::{
            ContainerNode, ContainerSplit, FindTreeResult, FoundNode, Node, NodeId, NodeVisitor,
            SizedNode, ToplevelNode, WorkspaceNode,
        },
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
            linkedlist::LinkedList,
            numcell::NumCell,
            smallmap::SmallMap,
        },
        wire::{wl_surface::*, WlOutputId, WlSurfaceId},
        xkbcommon::ModifierState,
    },
    ahash::AHashMap,
    jay_config::Direction,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        ops::{Deref, DerefMut},
        rc::Rc,
    },
    thiserror::Error,
};

#[allow(dead_code)]
const INVALID_SCALE: u32 = 0;
#[allow(dead_code)]
const INVALID_TRANSFORM: u32 = 1;
#[allow(dead_code)]
const INVALID_SIZE: u32 = 2;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SurfaceRole {
    None,
    Subsurface,
    XdgSurface,
    Cursor,
    DndIcon,
    ZwlrLayerSurface,
    XSurface,
}

impl SurfaceRole {
    fn name(self) -> &'static str {
        match self {
            SurfaceRole::None => "none",
            SurfaceRole::Subsurface => "subsurface",
            SurfaceRole::XdgSurface => "xdg_surface",
            SurfaceRole::Cursor => "cursor",
            SurfaceRole::DndIcon => "dnd_icon",
            SurfaceRole::ZwlrLayerSurface => "zwlr_layer_surface",
            SurfaceRole::XSurface => "xwayland surface",
        }
    }
}

pub struct WlSurface {
    pub id: WlSurfaceId,
    pub node_id: SurfaceNodeId,
    pub client: Rc<Client>,
    visible: Cell<bool>,
    role: Cell<SurfaceRole>,
    pending: PendingState,
    input_region: Cell<Option<Rc<Region>>>,
    opaque_region: Cell<Option<Rc<Region>>>,
    pub extents: Cell<Rect>,
    pub buffer_abs_pos: Cell<Rect>,
    pub need_extents_update: Cell<bool>,
    pub buffer: CloneCell<Option<Rc<WlBuffer>>>,
    pub buf_x: NumCell<i32>,
    pub buf_y: NumCell<i32>,
    pub children: RefCell<Option<Box<ParentData>>>,
    ext: CloneCell<Rc<dyn SurfaceExt>>,
    pub frame_requests: RefCell<Vec<Rc<WlCallback>>>,
    seat_state: NodeSeatState,
    toplevel: CloneCell<Option<Rc<dyn ToplevelNode>>>,
    cursors: SmallMap<SeatId, Rc<CursorSurface>, 1>,
    pub dnd_icons: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    pub tracker: Tracker<Self>,
}

impl Debug for WlSurface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WlSurface").finish_non_exhaustive()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum CommitContext {
    RootCommit,
    ChildCommit,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum CommitAction {
    ContinueCommit,
    AbortCommit,
}

trait SurfaceExt {
    fn pre_commit(self: Rc<Self>, ctx: CommitContext) -> Result<CommitAction, WlSurfaceError> {
        let _ = ctx;
        Ok(CommitAction::ContinueCommit)
    }

    fn post_commit(self: Rc<Self>) {
        // nothing
    }

    fn is_some(&self) -> bool {
        true
    }

    fn on_surface_destroy(&self) -> Result<(), WlSurfaceError> {
        if self.is_some() {
            Err(WlSurfaceError::ReloObjectStillExists)
        } else {
            Ok(())
        }
    }

    fn update_subsurface_parent_extents(&self) {
        // nothing
    }

    fn subsurface_parent(&self) -> Option<Rc<WlSurface>> {
        None
    }

    fn extents_changed(&self) {
        // nothing
    }

    fn into_subsurface(self: Rc<Self>) -> Option<Rc<WlSubsurface>> {
        None
    }

    fn accepts_kb_focus(&self) -> bool {
        true
    }
}

pub struct NoneSurfaceExt;

impl SurfaceExt for NoneSurfaceExt {
    fn is_some(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct PendingState {
    buffer: Cell<Option<Option<(i32, i32, Rc<WlBuffer>)>>>,
    opaque_region: Cell<Option<Option<Rc<Region>>>>,
    input_region: Cell<Option<Option<Rc<Region>>>>,
    frame_request: RefCell<Vec<Rc<WlCallback>>>,
}

#[derive(Default)]
pub struct ParentData {
    subsurfaces: AHashMap<WlSurfaceId, Rc<WlSubsurface>>,
    pub below: LinkedList<StackElement>,
    pub above: LinkedList<StackElement>,
}

pub struct StackElement {
    pub pending: Cell<bool>,
    pub sub_surface: Rc<WlSubsurface>,
}

impl WlSurface {
    pub fn new(id: WlSurfaceId, client: &Rc<Client>) -> Self {
        Self {
            id,
            node_id: client.state.node_ids.next(),
            client: client.clone(),
            visible: Cell::new(false),
            role: Cell::new(SurfaceRole::None),
            pending: Default::default(),
            input_region: Cell::new(None),
            opaque_region: Cell::new(None),
            extents: Default::default(),
            buffer_abs_pos: Cell::new(Default::default()),
            need_extents_update: Cell::new(false),
            buffer: CloneCell::new(None),
            buf_x: Default::default(),
            buf_y: Default::default(),
            children: Default::default(),
            ext: CloneCell::new(client.state.none_surface_ext.clone()),
            frame_requests: RefCell::new(vec![]),
            seat_state: Default::default(),
            toplevel: Default::default(),
            cursors: Default::default(),
            dnd_icons: Default::default(),
            tracker: Default::default(),
        }
    }

    fn set_absolute_position(&self, x1: i32, y1: i32) {
        self.buffer_abs_pos
            .set(self.buffer_abs_pos.get().at_point(x1, y1));
        if let Some(children) = self.children.borrow_mut().deref_mut() {
            for ss in children.subsurfaces.values() {
                let pos = ss.position.get();
                ss.surface
                    .set_absolute_position(x1 + pos.x1(), y1 + pos.y1());
            }
        }
    }

    pub fn is_cursor(&self) -> bool {
        self.role.get() == SurfaceRole::Cursor
    }

    pub fn get_cursor(
        self: &Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
    ) -> Result<Rc<CursorSurface>, WlSurfaceError> {
        if let Some(cursor) = self.cursors.get(&seat.id()) {
            return Ok(cursor);
        }
        self.set_role(SurfaceRole::Cursor)?;
        let cursor = Rc::new(CursorSurface::new(seat, self));
        track!(self.client, cursor);
        cursor.handle_buffer_change();
        self.cursors.insert(seat.id(), cursor.clone());
        Ok(cursor)
    }

    pub fn accepts_kb_focus(&self) -> bool {
        match self.toplevel.get() {
            Some(tl) => tl.accepts_keyboard_focus(),
            _ => self.ext.get().accepts_kb_focus(),
        }
    }

    fn send_enter(&self, output: WlOutputId) {
        self.client.event(Enter {
            self_id: self.id,
            output,
        })
    }

    fn set_toplevel(&self, tl: Option<Rc<dyn ToplevelNode>>) {
        let ch = self.children.borrow();
        if let Some(ch) = &*ch {
            for ss in ch.subsurfaces.values() {
                ss.surface.set_toplevel(tl.clone());
            }
        }
        if self.seat_state.is_active() {
            if let Some(tl) = &tl {
                tl.surface_active_changed(true);
            }
        }
        self.toplevel.set(tl);
    }

    pub fn set_role(&self, role: SurfaceRole) -> Result<(), WlSurfaceError> {
        use SurfaceRole::*;
        match (self.role.get(), role) {
            (None, _) => {}
            (old, new) if old == new => {}
            (old, new) => {
                return Err(WlSurfaceError::IncompatibleRole {
                    id: self.id,
                    old,
                    new,
                })
            }
        }
        self.role.set(role);
        Ok(())
    }

    fn unset_ext(&self) {
        self.ext.set(self.client.state.none_surface_ext.clone());
    }

    fn calculate_extents(&self) {
        let old_extents = self.extents.get();
        let mut extents = Rect::new_empty(0, 0);
        if let Some(b) = self.buffer.get() {
            extents = b.rect;
        }
        let children = self.children.borrow();
        if let Some(children) = &*children {
            for ss in children.subsurfaces.values() {
                let ce = ss.surface.extents.get();
                if !ce.is_empty() {
                    let cp = ss.position.get();
                    let ce = ce.move_(cp.x1(), cp.y1());
                    extents = if extents.is_empty() {
                        ce
                    } else {
                        extents.union(ce)
                    };
                }
            }
        }
        self.extents.set(extents);
        self.need_extents_update.set(false);
        if old_extents != extents {
            self.ext.get().extents_changed()
        }
    }

    pub fn get_root(self: &Rc<Self>) -> Rc<WlSurface> {
        let mut root = self.clone();
        loop {
            if let Some(parent) = root.ext.get().subsurface_parent() {
                root = parent;
                continue;
            }
            break;
        }
        root
    }

    fn parse<'a, T: RequestParser<'a>>(
        &self,
        parser: MsgParser<'_, 'a>,
    ) -> Result<T, MsgParserError> {
        self.client.parse(self, parser)
    }

    fn unset_cursors(&self) {
        while let Some((_, cursor)) = self.cursors.pop() {
            cursor.handle_surface_destroy();
        }
    }

    fn unset_dnd_icons(&self) {
        while let Some((_, seat)) = self.dnd_icons.pop() {
            seat.remove_dnd_icon()
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.parse(parser)?;
        self.unset_dnd_icons();
        self.unset_cursors();
        self.ext.get().on_surface_destroy()?;
        self.node_destroy(true);
        {
            let mut children = self.children.borrow_mut();
            if let Some(children) = &mut *children {
                for ss in children.subsurfaces.values() {
                    ss.surface.unset_ext();
                }
            }
            *children = None;
        }
        if let Some(buffer) = self.buffer.set(None) {
            if !buffer.destroyed() {
                buffer.send_release();
            }
        }
        self.frame_requests.borrow_mut().clear();
        self.toplevel.set(None);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn attach(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), AttachError> {
        let req: Attach = self.parse(parser)?;
        let buf = if req.buffer.is_some() {
            Some((req.x, req.y, self.client.lookup(req.buffer)?))
        } else {
            None
        };
        self.pending.buffer.set(Some(buf));
        Ok(())
    }

    fn damage(&self, parser: MsgParser<'_, '_>) -> Result<(), DamageError> {
        let _req: Damage = self.parse(parser)?;
        Ok(())
    }

    fn frame(&self, parser: MsgParser<'_, '_>) -> Result<(), FrameError> {
        let req: Frame = self.parse(parser)?;
        let cb = Rc::new(WlCallback::new(req.callback, &self.client));
        track!(self.client, cb);
        self.client.add_client_obj(&cb)?;
        self.pending.frame_request.borrow_mut().push(cb);
        Ok(())
    }

    fn set_opaque_region(&self, parser: MsgParser<'_, '_>) -> Result<(), SetOpaqueRegionError> {
        let region: SetOpaqueRegion = self.parse(parser)?;
        let region = if region.region.is_some() {
            Some(self.client.lookup(region.region)?.region())
        } else {
            None
        };
        self.pending.opaque_region.set(Some(region));
        Ok(())
    }

    fn set_input_region(&self, parser: MsgParser<'_, '_>) -> Result<(), SetInputRegionError> {
        let req: SetInputRegion = self.parse(parser)?;
        let region = if req.region.is_some() {
            Some(self.client.lookup(req.region)?.region())
        } else {
            None
        };
        self.pending.input_region.set(Some(region));
        Ok(())
    }

    fn do_commit(self: &Rc<Self>, ctx: CommitContext) -> Result<(), WlSurfaceError> {
        let ext = self.ext.get();
        if ext.clone().pre_commit(ctx)? == CommitAction::AbortCommit {
            return Ok(());
        }
        {
            let children = self.children.borrow();
            if let Some(children) = children.deref() {
                for child in children.subsurfaces.values() {
                    child.surface.do_commit(CommitContext::ChildCommit)?;
                }
            }
        }
        if let Some(buffer_change) = self.pending.buffer.take() {
            let mut old_size = None;
            let mut new_size = None;
            if let Some(buffer) = self.buffer.take() {
                old_size = Some(buffer.rect);
                if !buffer.destroyed() {
                    buffer.send_release();
                }
            }
            if let Some((dx, dy, buffer)) = buffer_change {
                let _ = buffer.update_texture();
                new_size = Some(buffer.rect);
                self.buffer_abs_pos.set(
                    self.buffer_abs_pos
                        .get()
                        .with_size(buffer.rect.width(), buffer.rect.height())
                        .unwrap(),
                );
                self.buffer.set(Some(buffer));
                self.buf_x.fetch_add(dx);
                self.buf_y.fetch_add(dy);
                if (dx, dy) != (0, 0) {
                    self.need_extents_update.set(true);
                    for (_, cursor) in &self.cursors {
                        cursor.dec_hotspot(dx, dy);
                    }
                }
            } else {
                self.buf_x.set(0);
                self.buf_y.set(0);
                for (_, cursor) in &self.cursors {
                    cursor.set_hotspot(0, 0);
                }
            }
            if old_size != new_size {
                self.need_extents_update.set(true);
            }
            for (_, cursor) in &self.cursors {
                cursor.handle_buffer_change();
            }
        }
        {
            let mut pfr = self.pending.frame_request.borrow_mut();
            self.frame_requests.borrow_mut().extend(pfr.drain(..));
        }
        {
            if let Some(region) = self.pending.input_region.take() {
                self.input_region.set(region);
            }
            if let Some(region) = self.pending.opaque_region.take() {
                self.opaque_region.set(region);
            }
        }
        if self.need_extents_update.get() {
            self.calculate_extents();
        }
        ext.post_commit();
        Ok(())
    }

    fn commit(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), CommitError> {
        let _req: Commit = self.parse(parser)?;
        self.do_commit(CommitContext::RootCommit)?;
        Ok(())
    }

    fn set_buffer_transform(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), SetBufferTransformError> {
        let _req: SetBufferTransform = self.parse(parser)?;
        Ok(())
    }

    fn set_buffer_scale(&self, parser: MsgParser<'_, '_>) -> Result<(), SetBufferScaleError> {
        let _req: SetBufferScale = self.parse(parser)?;
        Ok(())
    }

    fn damage_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), DamageBufferError> {
        let _req: DamageBuffer = self.parse(parser)?;
        Ok(())
    }

    fn find_surface_at(self: &Rc<Self>, x: i32, y: i32) -> Option<(Rc<Self>, i32, i32)> {
        let buffer = match self.buffer.get() {
            Some(b) => b,
            _ => return None,
        };
        let children = self.children.borrow();
        let children = match children.deref() {
            Some(c) => c,
            _ => {
                return if buffer.rect.contains(x, y) {
                    Some((self.clone(), x, y))
                } else {
                    None
                };
            }
        };
        let ss = |c: &LinkedList<StackElement>| {
            for child in c.rev_iter() {
                if child.pending.get() {
                    continue;
                }
                let pos = child.sub_surface.position.get();
                if pos.contains(x, y) {
                    let (x, y) = pos.translate(x, y);
                    if let Some(res) = child.sub_surface.surface.find_surface_at(x, y) {
                        return Some(res);
                    }
                }
            }
            None
        };
        if let Some(res) = ss(&children.above) {
            return Some(res);
        }
        if buffer.rect.contains(x, y) {
            return Some((self.clone(), x, y));
        }
        if let Some(res) = ss(&children.below) {
            return Some(res);
        }
        None
    }

    fn find_tree_at_(self: &Rc<Self>, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        match self.find_surface_at(x, y) {
            Some((node, x, y)) => {
                tree.push(FoundNode { node, x, y });
                FindTreeResult::AcceptsInput
            }
            _ => FindTreeResult::Other,
        }
    }

    fn send_seat_release_events(&self) {
        self.seat_state
            .for_each_pointer_focus(|s| s.leave_surface(self));
        self.seat_state
            .for_each_kb_focus(|s| s.unfocus_surface(self));
    }
}

object_base! {
    WlSurface, WlSurfaceError;

    DESTROY => destroy,
    ATTACH => attach,
    DAMAGE => damage,
    FRAME => frame,
    SET_OPAQUE_REGION => set_opaque_region,
    SET_INPUT_REGION => set_input_region,
    COMMIT => commit,
    SET_BUFFER_TRANSFORM => set_buffer_transform,
    SET_BUFFER_SCALE => set_buffer_scale,
    DAMAGE_BUFFER => damage_buffer,
}

impl Object for WlSurface {
    fn num_requests(&self) -> u32 {
        DAMAGE_BUFFER + 1
    }

    fn break_loops(&self) {
        self.unset_dnd_icons();
        self.unset_cursors();
        self.node_destroy(true);
        *self.children.borrow_mut() = None;
        self.unset_ext();
        mem::take(self.frame_requests.borrow_mut().deref_mut());
        self.buffer.set(None);
        self.toplevel.set(None);
    }
}

dedicated_add_obj!(WlSurface, WlSurfaceId, surfaces);

tree_id!(SurfaceNodeId);
impl SizedNode for WlSurface {
    fn id(&self) -> NodeId {
        self.node_id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        let children = self.children.borrow();
        if let Some(ch) = children.deref() {
            for ss in ch.subsurfaces.values() {
                ss.surface.node_destroy(false);
            }
        }
        if let Some(tl) = self.toplevel.get() {
            let data = tl.data();
            let mut remove = vec![];
            for (seat, s) in data.focus_surface.iter() {
                if s.id == self.id {
                    remove.push(seat);
                }
            }
            for seat in remove {
                data.focus_surface.remove(&seat);
            }
            if self.seat_state.is_active() {
                tl.surface_active_changed(false);
            }
        }
        self.send_seat_release_events();
        self.seat_state.destroy_node(self);
    }

    fn visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let children = self.children.borrow_mut();
        if let Some(c) = children.deref() {
            for child in c.subsurfaces.values() {
                visitor.visit_surface(&child.surface);
            }
        }
    }

    fn visible(&self) -> bool {
        self.visible.get()
    }

    fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        let children = self.children.borrow_mut();
        if let Some(children) = children.deref() {
            for child in children.subsurfaces.values() {
                child.surface.node_set_visible(visible);
            }
        }
        if !visible {
            self.send_seat_release_events();
        }
        self.seat_state.set_visible(self, visible);
    }

    fn get_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        if let Some(tl) = self.toplevel.get() {
            return tl.as_node().node_get_workspace();
        }
        None
    }

    fn get_parent_mono(&self) -> Option<bool> {
        self.toplevel
            .get()
            .and_then(|t| t.parent())
            .and_then(|p| p.node_get_mono())
    }

    fn get_parent_split(&self) -> Option<ContainerSplit> {
        self.toplevel
            .get()
            .and_then(|t| t.parent())
            .and_then(|p| p.node_get_split())
    }

    fn set_parent_mono(&self, mono: bool) {
        if let Some(tl) = self.toplevel.get() {
            if let Some(pn) = tl.parent() {
                let node = if mono { Some(tl.as_node()) } else { None };
                pn.node_set_mono(node)
            }
        }
    }

    fn set_parent_split(&self, split: ContainerSplit) {
        if let Some(tl) = self.toplevel.get() {
            if let Some(pn) = tl.parent() {
                pn.node_set_split(split);
            }
        }
    }

    fn create_split(self: &Rc<Self>, split: ContainerSplit) {
        let tl = match self.toplevel.get() {
            Some(tl) => tl,
            _ => return,
        };
        let ws = match tl.as_node().node_get_workspace() {
            Some(ws) => ws,
            _ => return,
        };
        let pn = match tl.parent() {
            Some(pn) => pn,
            _ => return,
        };
        let cn = ContainerNode::new(
            &self.client.state,
            &ws,
            pn.clone(),
            tl.clone().into_node(),
            split,
        );
        pn.node_replace_child(tl.as_node(), cn);
    }

    fn close(&self) {
        if let Some(tl) = self.toplevel.get() {
            tl.close();
        }
    }

    fn move_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        if let Some(tl) = self.toplevel.get() {
            if let Some(pn) = tl.parent() {
                pn.node_move_focus_from_child(seat, tl.as_node(), direction);
            }
        }
    }

    fn move_self(self: &Rc<Self>, direction: Direction) {
        if let Some(tl) = self.toplevel.get() {
            if let Some(pn) = tl.parent() {
                pn.node_move_child(tl.into_node(), direction);
            }
        }
    }

    fn absolute_position(&self) -> Rect {
        self.buffer_abs_pos.get()
    }

    fn active_changed(&self, active: bool) {
        if let Some(tl) = self.toplevel.get() {
            tl.surface_active_changed(active);
        }
    }

    fn key(&self, seat: &WlSeatGlobal, key: u32, state: u32) {
        seat.key_surface(self, key, state);
    }

    fn mods(&self, seat: &WlSeatGlobal, mods: ModifierState) {
        seat.mods_surface(self, mods);
    }

    fn button(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState, serial: u32) {
        seat.button_surface(&self, button, state, serial);
    }

    fn axis_event(self: &Rc<Self>, seat: &WlSeatGlobal, event: &PendingScroll) {
        seat.scroll_surface(&*self, event);
    }

    fn focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        if let Some(tl) = self.toplevel.get() {
            tl.data().focus_surface.insert(seat.id(), self.clone());
            tl.activate();
        }
        seat.focus_surface(&self);
    }

    fn focus_parent(&self, seat: &Rc<WlSeatGlobal>) {
        if let Some(tl) = self.toplevel.get() {
            tl.parent().map(|p| p.node_focus_self(seat));
        }
    }

    fn toggle_floating(self: &Rc<Self>, _seat: &Rc<WlSeatGlobal>) {
        if let Some(tl) = self.toplevel.get() {
            tl.toggle_floating();
        }
    }

    fn unfocus(&self, seat: &WlSeatGlobal) {
        seat.unfocus_surface(self);
    }

    fn leave(&self, seat: &WlSeatGlobal) {
        seat.leave_surface(self);
    }

    fn pointer_enter(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        seat.enter_surface(&self, x, y)
    }

    fn pointer_motion(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        seat.motion_surface(&*self, x, y)
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_surface(self, x, y);
    }

    fn client(&self) -> Option<Rc<Client>> {
        Some(self.client.clone())
    }

    fn into_surface(self: &Rc<Self>) -> Option<Rc<WlSurface>> {
        Some(self.clone())
    }

    fn dnd_drop(&self, dnd: &Dnd) {
        dnd.seat.dnd_surface_drop(self, dnd);
    }

    fn dnd_leave(&self, dnd: &Dnd) {
        dnd.seat.dnd_surface_leave(self, dnd);
    }

    fn dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed, serial: u32) {
        dnd.seat.dnd_surface_enter(self, dnd, x, y, serial);
    }

    fn dnd_motion(&self, dnd: &Dnd, x: Fixed, y: Fixed) {
        dnd.seat.dnd_surface_motion(self, dnd, x, y);
    }

    fn is_xwayland_surface(&self) -> bool {
        self.client.is_xwayland
    }
}

#[derive(Debug, Error)]
pub enum WlSurfaceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ZwlrLayerSurfaceV1Error(Box<ZwlrLayerSurfaceV1Error>),
    #[error(transparent)]
    XdgSurfaceError(Box<XdgSurfaceError>),
    #[error("Could not process `destroy` request")]
    DestroyError(#[source] Box<DestroyError>),
    #[error("Could not process `attach` request")]
    AttachError(#[source] Box<AttachError>),
    #[error("Could not process `damage` request")]
    DamageError(#[source] Box<DamageError>),
    #[error("Could not process `frame` request")]
    FrameError(#[source] Box<FrameError>),
    #[error("Could not process `set_opaque_region` request")]
    SetOpaqueRegionError(#[source] Box<SetOpaqueRegionError>),
    #[error("Could not process `set_input_region` request")]
    SetInputRegionError(#[source] Box<SetInputRegionError>),
    #[error("Could not process `commit` request")]
    CommitError(#[source] Box<CommitError>),
    #[error("Could not process `set_buffer_transform` request")]
    SetBufferTransformError(#[source] Box<SetBufferTransformError>),
    #[error("Could not process `set_buffer_scale_error` request")]
    SetBufferScaleError(#[source] Box<SetBufferScaleError>),
    #[error("Could not process `damage_buffer` request")]
    DamageBufferError(#[source] Box<DamageBufferError>),
    #[error("Surface {} cannot be assigned the role {} because it already has the role {}", .id, .new.name(), .old.name())]
    IncompatibleRole {
        id: WlSurfaceId,
        old: SurfaceRole,
        new: SurfaceRole,
    },
    #[error("Cannot destroy a `wl_surface` before its role object")]
    ReloObjectStillExists,
}
efrom!(WlSurfaceError, ClientError);
efrom!(WlSurfaceError, XdgSurfaceError);
efrom!(WlSurfaceError, DestroyError);
efrom!(WlSurfaceError, AttachError);
efrom!(WlSurfaceError, DamageError);
efrom!(WlSurfaceError, FrameError);
efrom!(WlSurfaceError, SetOpaqueRegionError);
efrom!(WlSurfaceError, SetInputRegionError);
efrom!(WlSurfaceError, CommitError);
efrom!(WlSurfaceError, SetBufferTransformError);
efrom!(WlSurfaceError, SetBufferScaleError);
efrom!(WlSurfaceError, DamageBufferError);
efrom!(WlSurfaceError, ZwlrLayerSurfaceV1Error);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);
efrom!(DestroyError, WlSurfaceError);

#[derive(Debug, Error)]
pub enum AttachError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(AttachError, ParseFailed, MsgParserError);
efrom!(AttachError, ClientError);

#[derive(Debug, Error)]
pub enum DamageError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(DamageError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(FrameError, ParseFailed, MsgParserError);
efrom!(FrameError, ClientError);

#[derive(Debug, Error)]
pub enum SetOpaqueRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetOpaqueRegionError, ParseFailed, MsgParserError);
efrom!(SetOpaqueRegionError, ClientError);

#[derive(Debug, Error)]
pub enum SetInputRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetInputRegionError, ParseFailed, MsgParserError);
efrom!(SetInputRegionError, ClientError);

#[derive(Debug, Error)]
pub enum CommitError {
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CommitError, WlSurfaceError);
efrom!(CommitError, ParseFailed, MsgParserError);
efrom!(CommitError, ClientError);

#[derive(Debug, Error)]
pub enum SetBufferTransformError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetBufferTransformError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SetBufferScaleError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetBufferScaleError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum DamageBufferError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(DamageBufferError, ParseFailed, MsgParserError);
