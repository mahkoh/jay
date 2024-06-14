use {
    crate::{
        backend::{AxisSource, KeyState, ScrollAxis, AXIS_120},
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            ipc,
            ipc::wl_data_source::WlDataSource,
            wl_seat::{
                wl_pointer::PendingScroll, Dnd, DroppedDnd, WlSeatError, WlSeatGlobal, BTN_LEFT,
                BTN_RIGHT, CHANGE_CURSOR_MOVED, CHANGE_TREE,
            },
            wl_surface::WlSurface,
            xdg_toplevel_drag_v1::XdgToplevelDragV1,
        },
        state::DeviceHandlerData,
        tree::{ContainingNode, FindTreeUsecase, FoundNode, Node, ToplevelNode, WorkspaceNode},
        utils::{clonecell::CloneCell, smallmap::SmallMap},
    },
    std::{
        cell::Cell,
        rc::{Rc, Weak},
    },
};

pub struct PointerOwnerHolder {
    default: Rc<SimplePointerOwner<DefaultPointerUsecase>>,
    owner: CloneCell<Rc<dyn PointerOwner>>,
    pending_scroll: PendingScroll,
}

pub trait ToplevelSelector: 'static {
    fn set(&self, toplevel: Rc<dyn ToplevelNode>);
}

pub trait WorkspaceSelector: 'static {
    fn set(&self, ws: Rc<WorkspaceNode>);
}

impl Default for PointerOwnerHolder {
    fn default() -> Self {
        let default = Rc::new(SimplePointerOwner {
            usecase: DefaultPointerUsecase,
        });
        Self {
            default: default.clone(),
            owner: CloneCell::new(default.clone()),
            pending_scroll: Default::default(),
        }
    }
}

impl PointerOwnerHolder {
    pub fn button(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, button: u32, state: KeyState) {
        self.owner.get().button(seat, time_usec, button, state)
    }

    pub fn axis_source(&self, axis_source: AxisSource) {
        self.pending_scroll.source.set(Some(axis_source as _));
    }

    pub fn axis_120(&self, delta: i32, axis: ScrollAxis, inverted: bool) {
        self.pending_scroll.v120[axis as usize].set(Some(delta));
        self.pending_scroll.inverted[axis as usize].set(inverted);
    }

    pub fn axis_px(&self, delta: Fixed, axis: ScrollAxis, inverted: bool) {
        self.pending_scroll.px[axis as usize].set(Some(delta));
        self.pending_scroll.inverted[axis as usize].set(inverted);
    }

    pub fn axis_stop(&self, axis: ScrollAxis) {
        self.pending_scroll.stop[axis as usize].set(true);
    }

    pub fn frame(&self, dev: &DeviceHandlerData, seat: &Rc<WlSeatGlobal>, time_usec: u64) {
        self.pending_scroll.time_usec.set(time_usec);
        let pending = self.pending_scroll.take();
        for axis in 0..2 {
            if let Some(dist) = pending.v120[axis].get() {
                let px = (dist as f64 / AXIS_120 as f64) * dev.px_per_scroll_wheel.get();
                pending.px[axis].set(Some(Fixed::from_f64(px)));
            }
        }
        seat.state.for_each_seat_tester(|t| {
            t.send_axis(seat.id, time_usec, &pending);
        });
        if let Some(node) = self.owner.get().axis_node(seat) {
            node.node_on_axis_event(seat, &pending);
        }
    }

    pub fn relative_motion(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        if let Some(n) = self.owner.get().axis_node(seat) {
            n.node_on_pointer_relative_motion(
                seat,
                time_usec,
                dx,
                dy,
                dx_unaccelerated,
                dy_unaccelerated,
            );
        }
    }

    pub fn apply_changes(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().apply_changes(seat)
    }

    pub fn start_drag(
        &self,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
        serial: u32,
    ) -> Result<(), WlSeatError> {
        self.owner
            .get()
            .start_drag(seat, origin, source, icon, serial)
    }

    pub fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().cancel_dnd(seat)
    }

    pub fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().revert_to_default(seat)
    }

    pub fn grab_node_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().grab_node_removed(seat);
    }

    pub fn dnd_target_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().dnd_target_removed(seat);
    }

    pub fn dnd_icon(&self) -> Option<Rc<WlSurface>> {
        self.owner.get().dnd_icon()
    }

    pub fn toplevel_drag(&self) -> Option<Rc<XdgToplevelDragV1>> {
        self.owner.get().toplevel_drag()
    }

    pub fn remove_dnd_icon(&self) {
        self.owner.get().remove_dnd_icon()
    }

    pub fn clear(&self) {
        self.owner.set(self.default.clone());
    }

    fn set_default_pointer_owner(&self, seat: &Rc<WlSeatGlobal>) {
        seat.pointer_owner.owner.set(self.default.clone());
        seat.changes.or_assign(CHANGE_CURSOR_MOVED);
    }

    fn select_element(&self, seat: &Rc<WlSeatGlobal>, usecase: impl SimplePointerOwnerUsecase) {
        self.revert_to_default(seat);
        if let Some(node) = seat.pointer_stack.borrow().last() {
            usecase.node_focus(seat, node);
        }
        self.owner.set(Rc::new(SimplePointerOwner { usecase }));
        seat.trigger_tree_changed();
    }

    pub fn select_toplevel(&self, seat: &Rc<WlSeatGlobal>, selector: impl ToplevelSelector) {
        let usecase = Rc::new(SelectToplevelUsecase {
            seat: Rc::downgrade(seat),
            selector,
            latest: Default::default(),
        });
        self.select_element(seat, usecase)
    }

    pub fn select_workspace(&self, seat: &Rc<WlSeatGlobal>, selector: impl WorkspaceSelector) {
        let usecase = Rc::new(SelectWorkspaceUsecase {
            seat: Rc::downgrade(seat),
            selector,
            latest: Default::default(),
        });
        self.select_element(seat, usecase)
    }

    pub fn set_window_management_enabled(&self, seat: &Rc<WlSeatGlobal>, enabled: bool) {
        let owner = self.owner.get();
        if enabled {
            owner.enable_window_management(seat);
        } else {
            owner.disable_window_management(seat);
        }
    }
}

trait PointerOwner {
    fn button(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, button: u32, state: KeyState);
    fn axis_node(&self, seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>>;
    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>);
    fn start_drag(
        &self,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
        serial: u32,
    ) -> Result<(), WlSeatError> {
        let _ = origin;
        let _ = icon;
        let _ = serial;
        if let Some(src) = source {
            src.send_cancelled(seat);
        }
        Ok(())
    }
    fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>) {
        seat.dropped_dnd.borrow_mut().take();
    }
    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>);
    fn grab_node_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.revert_to_default(seat);
    }
    fn dnd_target_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.cancel_dnd(seat);
    }
    fn dnd_icon(&self) -> Option<Rc<WlSurface>> {
        None
    }
    fn toplevel_drag(&self) -> Option<Rc<XdgToplevelDragV1>> {
        None
    }
    fn remove_dnd_icon(&self) {
        // nothing
    }
    fn enable_window_management(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }
    fn disable_window_management(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }
}

struct SimplePointerOwner<T> {
    usecase: T,
}

struct SimpleGrabPointerOwner<T> {
    usecase: T,
    buttons: SmallMap<u32, (), 1>,
    node: Rc<dyn Node>,
    serial: u32,
}

struct DndPointerOwner {
    button: u32,
    dnd: Dnd,
    target: CloneCell<Rc<dyn Node>>,
    icon: CloneCell<Option<Rc<WlSurface>>>,
    pos_x: Cell<Fixed>,
    pos_y: Cell<Fixed>,
}

#[derive(Copy, Clone)]
struct DefaultPointerUsecase;

struct SelectToplevelUsecase<S: ?Sized> {
    seat: Weak<WlSeatGlobal>,
    latest: CloneCell<Option<Rc<dyn ToplevelNode>>>,
    selector: S,
}

struct SelectWorkspaceUsecase<S: ?Sized> {
    seat: Weak<WlSeatGlobal>,
    latest: CloneCell<Option<Rc<WorkspaceNode>>>,
    selector: S,
}

#[derive(Copy, Clone)]
struct WindowManagementUsecase;

impl<T: SimplePointerOwnerUsecase> PointerOwner for SimplePointerOwner<T> {
    fn button(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, button: u32, state: KeyState) {
        if state != KeyState::Pressed {
            return;
        }
        let pn = match seat.pointer_node() {
            Some(n) => n,
            _ => return,
        };
        if self.usecase.default_button(self, seat, button, &pn) {
            return;
        }
        let serial = seat.state.next_serial(pn.node_client().as_deref());
        seat.pointer_owner
            .owner
            .set(Rc::new(SimpleGrabPointerOwner {
                usecase: self.usecase.clone(),
                buttons: SmallMap::new_with(button, ()),
                node: pn.clone(),
                serial,
            }));
        pn.node_seat_state().add_pointer_grab(seat);
        pn.node_on_button(seat, time_usec, button, state, serial);
    }

    fn axis_node(&self, seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>> {
        seat.pointer_node()
    }

    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>) {
        let (x, y) = seat.pointer_cursor.position();
        let mut found_tree = seat.found_tree.borrow_mut();
        let mut stack = seat.pointer_stack.borrow_mut();
        let x_int = x.round_down();
        let y_int = y.round_down();
        found_tree.push(FoundNode {
            node: seat.state.root.clone(),
            x: x_int,
            y: y_int,
        });
        seat.state
            .root
            .node_find_tree_at(x_int, y_int, &mut found_tree, T::FIND_TREE_USECASE);
        let mut divergence = found_tree.len().min(stack.len());
        for (i, (found, stack)) in found_tree.iter().zip(stack.iter()).enumerate() {
            if found.node.node_id() != stack.node_id() {
                divergence = i;
                break;
            }
        }
        let psm = seat.pointer_stack_modified.replace(false);
        if !psm && (stack.len(), found_tree.len()) == (divergence, divergence) {
            if let Some(node) = found_tree.last() {
                node.node.clone().node_on_pointer_motion(
                    seat,
                    x.apply_fract(node.x),
                    y.apply_fract(node.y),
                );
            }
        } else {
            if let Some(last) = stack.last() {
                last.node_on_pointer_unfocus(seat);
            }
            for old in stack.drain(divergence..).rev() {
                old.node_on_leave(seat);
                old.node_seat_state().leave(seat);
            }
            if found_tree.len() == divergence {
                if let Some(node) = found_tree.last() {
                    node.node.clone().node_on_pointer_motion(
                        seat,
                        x.apply_fract(node.x),
                        y.apply_fract(node.y),
                    );
                }
            } else {
                for new in found_tree.drain(divergence..) {
                    new.node.node_seat_state().enter(seat);
                    new.node.clone().node_on_pointer_enter(
                        seat,
                        x.apply_fract(new.x),
                        y.apply_fract(new.y),
                    );
                    stack.push(new.node);
                }
            }
            if let Some(node) = stack.last() {
                node.node_on_pointer_focus(seat);
                self.usecase.node_focus(seat, node);
            }
        }
        found_tree.clear();
    }

    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        if !T::IS_DEFAULT {
            seat.pointer_owner.set_default_pointer_owner(seat);
            seat.trigger_tree_changed();
            seat.state.damage();
        }
    }

    fn enable_window_management(&self, seat: &Rc<WlSeatGlobal>) {
        if !T::IS_DEFAULT {
            return;
        }
        seat.pointer_owner.owner.set(Rc::new(SimplePointerOwner {
            usecase: WindowManagementUsecase,
        }));
        seat.changes.or_assign(CHANGE_TREE);
        seat.apply_changes();
    }

    fn disable_window_management(&self, seat: &Rc<WlSeatGlobal>) {
        self.usecase.disable_window_management(seat);
    }
}

impl<T: SimplePointerOwnerUsecase> PointerOwner for SimpleGrabPointerOwner<T> {
    fn button(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, button: u32, state: KeyState) {
        match state {
            KeyState::Released => {
                self.buttons.remove(&button);
                if self.buttons.is_empty() {
                    self.node.node_seat_state().remove_pointer_grab(seat);
                    // log::info!("button");
                    self.usecase.release_grab(seat);
                    seat.tree_changed.trigger();
                }
            }
            KeyState::Pressed => {
                self.buttons.insert(button, ());
            }
        }
        let serial = seat.state.next_serial(self.node.node_client().as_deref());
        self.node
            .clone()
            .node_on_button(seat, time_usec, button, state, serial);
    }

    fn axis_node(&self, _seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>> {
        Some(self.node.clone())
    }

    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>) {
        let (x, y) = seat.pointer_cursor.position();
        let pos = self.node.node_absolute_position();
        let (x_int, y_int) = pos.translate(x.round_down(), y.round_down());
        // log::info!("apply_changes");
        self.node
            .clone()
            .node_on_pointer_motion(seat, x.apply_fract(x_int), y.apply_fract(y_int));
    }

    fn start_drag(
        &self,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        src: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
        serial: u32,
    ) -> Result<(), WlSeatError> {
        self.usecase
            .start_drag(self, seat, origin, src, icon, serial)
    }

    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.node.node_seat_state().remove_pointer_grab(seat);
        seat.pointer_owner.set_default_pointer_owner(seat);
    }
}

impl PointerOwner for DndPointerOwner {
    fn button(&self, seat: &Rc<WlSeatGlobal>, _time_usec: u64, button: u32, state: KeyState) {
        if button != self.button || state != KeyState::Released {
            return;
        }
        let target = self.target.get();
        target.node_on_dnd_drop(&self.dnd);
        if let Some(src) = &self.dnd.src {
            src.on_drop(seat);
        }
        let should_drop = match &self.dnd.src {
            None => true,
            Some(s) => s.can_drop(),
        };
        if should_drop {
            *seat.dropped_dnd.borrow_mut() = Some(DroppedDnd {
                dnd: self.dnd.clone(),
            });
        }
        target.node_on_dnd_leave(&self.dnd);
        target.node_seat_state().remove_dnd_target(seat);
        if !should_drop {
            if let Some(src) = &self.dnd.src {
                ipc::detach_seat(&**src, seat);
            }
        }
        if let Some(icon) = self.icon.get() {
            icon.set_dnd_icon_seat(seat.id(), None);
        }
        seat.pointer_owner.set_default_pointer_owner(seat);
        seat.tree_changed.trigger();
    }

    fn axis_node(&self, _seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>> {
        None
    }

    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>) {
        let (x, y) = seat.pointer_cursor.position();
        let (x_int, y_int) = (x.round_down(), y.round_down());
        let (node, x_int, y_int) = {
            let mut found_tree = seat.found_tree.borrow_mut();
            found_tree.push(FoundNode {
                node: seat.state.root.clone(),
                x: x_int,
                y: y_int,
            });
            seat.state
                .root
                .node_find_tree_at(x_int, y_int, &mut found_tree, FindTreeUsecase::None);
            let FoundNode { node, x, y } = found_tree.pop().unwrap();
            found_tree.clear();
            (node, x, y)
        };
        let (x, y) = (x.apply_fract(x_int), y.apply_fract(y_int));
        let mut target = self.target.get();
        if node.node_id() != target.node_id() {
            target.node_on_dnd_leave(&self.dnd);
            target.node_seat_state().remove_dnd_target(seat);
            target = node;
            target.node_on_dnd_enter(
                &self.dnd,
                x,
                y,
                seat.state.next_serial(target.node_client().as_deref()),
            );
            target.node_seat_state().add_dnd_target(seat);
            self.target.set(target);
        } else if (self.pos_x.get(), self.pos_y.get()) != (x, y) {
            node.node_on_dnd_motion(&self.dnd, seat.pos_time_usec.get(), x, y);
        }
        self.pos_x.set(x);
        self.pos_y.set(y);
    }

    fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>) {
        let target = self.target.get();
        target.node_on_dnd_leave(&self.dnd);
        target.node_seat_state().remove_dnd_target(seat);
        if let Some(src) = &self.dnd.src {
            ipc::detach_seat(&**src, seat);
        }
        if let Some(icon) = self.icon.get() {
            icon.set_dnd_icon_seat(seat.id(), None);
        }
        seat.pointer_owner.set_default_pointer_owner(seat);
        seat.tree_changed.trigger();
    }

    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.cancel_dnd(seat)
    }

    fn dnd_target_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.target.get().node_on_dnd_leave(&self.dnd);
        self.target.set(seat.state.root.clone());
        seat.state.tree_changed();
    }

    fn dnd_icon(&self) -> Option<Rc<WlSurface>> {
        self.icon.get()
    }

    fn toplevel_drag(&self) -> Option<Rc<XdgToplevelDragV1>> {
        if let Some(src) = &self.dnd.src {
            src.toplevel_drag.get()
        } else {
            None
        }
    }

    fn remove_dnd_icon(&self) {
        self.icon.set(None);
    }
}

trait SimplePointerOwnerUsecase: Sized + Clone + 'static {
    const FIND_TREE_USECASE: FindTreeUsecase;
    const IS_DEFAULT: bool;

    fn default_button(
        &self,
        spo: &SimplePointerOwner<Self>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        pn: &Rc<dyn Node>,
    ) -> bool;

    fn start_drag(
        &self,
        grab: &SimpleGrabPointerOwner<Self>,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        src: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
        serial: u32,
    ) -> Result<(), WlSeatError> {
        let _ = grab;
        let _ = origin;
        let _ = icon;
        let _ = serial;
        if let Some(src) = src {
            src.send_cancelled(seat);
        }
        Ok(())
    }

    fn release_grab(&self, seat: &Rc<WlSeatGlobal>);

    fn node_focus(&self, seat: &Rc<WlSeatGlobal>, node: &Rc<dyn Node>) {
        let _ = seat;
        let _ = node;
    }

    fn disable_window_management(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }
}

impl SimplePointerOwnerUsecase for DefaultPointerUsecase {
    const FIND_TREE_USECASE: FindTreeUsecase = FindTreeUsecase::None;
    const IS_DEFAULT: bool = true;

    fn default_button(
        &self,
        _spo: &SimplePointerOwner<Self>,
        _seat: &Rc<WlSeatGlobal>,
        _button: u32,
        _pn: &Rc<dyn Node>,
    ) -> bool {
        false
    }

    fn start_drag(
        &self,
        grab: &SimpleGrabPointerOwner<Self>,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        src: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
        serial: u32,
    ) -> Result<(), WlSeatError> {
        let button = match grab.buttons.iter().next() {
            Some((b, _)) => b,
            None => return Ok(()),
        };
        if grab.buttons.len() != 1 {
            return Ok(());
        }
        if serial != grab.serial {
            return Ok(());
        }
        if grab.node.node_id() != origin.node_id {
            return Ok(());
        }
        if let Some(icon) = &icon {
            icon.set_dnd_icon_seat(seat.id, Some(seat));
        }
        if let Some(new) = &src {
            ipc::attach_seat(&**new, seat, ipc::Role::Dnd)?;
            if let Some(drag) = new.toplevel_drag.get() {
                drag.start_drag();
            }
        }
        *seat.dropped_dnd.borrow_mut() = None;
        let pointer_owner = Rc::new(DndPointerOwner {
            button,
            dnd: Dnd {
                seat: seat.clone(),
                client: origin.client.clone(),
                src,
            },
            target: CloneCell::new(seat.state.root.clone()),
            icon: CloneCell::new(icon),
            pos_x: Cell::new(Fixed::from_int(0)),
            pos_y: Cell::new(Fixed::from_int(0)),
        });
        {
            let mut stack = seat.pointer_stack.borrow_mut();
            for node in stack.drain(1..).rev() {
                node.node_on_leave(seat);
                node.node_seat_state().leave(seat);
            }
        }
        grab.node.node_seat_state().remove_pointer_grab(seat);
        // {
        //     let old = seat.keyboard_node.set(seat.state.root.clone());
        //     old.seat_state().unfocus(seat);
        //     old.unfocus(seat);
        // }
        seat.pointer_owner.owner.set(pointer_owner.clone());
        pointer_owner.apply_changes(seat);
        Ok(())
    }

    fn release_grab(&self, seat: &Rc<WlSeatGlobal>) {
        seat.pointer_owner.set_default_pointer_owner(seat);
    }
}

trait NodeSelectorUsecase: Sized + 'static {
    const FIND_TREE_USECASE: FindTreeUsecase;

    fn default_button(
        self: &Rc<Self>,
        spo: &SimplePointerOwner<Rc<Self>>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        pn: &Rc<dyn Node>,
    ) -> bool;

    fn node_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, node: &Rc<dyn Node>);
}

impl<U: NodeSelectorUsecase + ?Sized> SimplePointerOwnerUsecase for Rc<U> {
    const FIND_TREE_USECASE: FindTreeUsecase = <U as NodeSelectorUsecase>::FIND_TREE_USECASE;
    const IS_DEFAULT: bool = false;

    fn default_button(
        &self,
        spo: &SimplePointerOwner<Self>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        pn: &Rc<dyn Node>,
    ) -> bool {
        <U as NodeSelectorUsecase>::default_button(self, spo, seat, button, pn)
    }

    fn release_grab(&self, seat: &Rc<WlSeatGlobal>) {
        seat.pointer_owner.owner.set(Rc::new(SimplePointerOwner {
            usecase: self.clone(),
        }));
        seat.changes.or_assign(CHANGE_CURSOR_MOVED);
    }

    fn node_focus(&self, seat: &Rc<WlSeatGlobal>, node: &Rc<dyn Node>) {
        <U as NodeSelectorUsecase>::node_focus(self, seat, node)
    }
}

impl<S: ToplevelSelector> NodeSelectorUsecase for SelectToplevelUsecase<S> {
    const FIND_TREE_USECASE: FindTreeUsecase = FindTreeUsecase::SelectToplevel;

    fn default_button(
        self: &Rc<Self>,
        spo: &SimplePointerOwner<Rc<Self>>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        pn: &Rc<dyn Node>,
    ) -> bool {
        let Some(tl) = pn.clone().node_into_toplevel() else {
            return false;
        };
        let selected_toplevel =
            button == BTN_RIGHT || (button == BTN_LEFT && !tl.tl_admits_children());
        if !selected_toplevel {
            return false;
        }
        self.selector.set(tl);
        spo.revert_to_default(seat);
        true
    }

    fn node_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, node: &Rc<dyn Node>) {
        let mut damage = false;
        let tl = node.clone().node_into_toplevel();
        if let Some(tl) = &tl {
            tl.tl_data().render_highlight.fetch_add(1);
            if !tl.tl_admits_children() {
                seat.pointer_cursor().set_known(KnownCursor::Pointer);
            }
            damage = true;
        }
        if let Some(prev) = self.latest.set(tl) {
            prev.tl_data().render_highlight.fetch_sub(1);
            damage = true;
        }
        if damage {
            seat.state.damage();
        }
    }
}

impl<S: ?Sized> Drop for SelectToplevelUsecase<S> {
    fn drop(&mut self) {
        if let Some(prev) = self.latest.take() {
            prev.tl_data().render_highlight.fetch_sub(1);
            if let Some(seat) = self.seat.upgrade() {
                seat.state.damage();
            }
        }
    }
}

impl<S: WorkspaceSelector> NodeSelectorUsecase for SelectWorkspaceUsecase<S> {
    const FIND_TREE_USECASE: FindTreeUsecase = FindTreeUsecase::SelectWorkspace;

    fn default_button(
        self: &Rc<Self>,
        spo: &SimplePointerOwner<Rc<Self>>,
        seat: &Rc<WlSeatGlobal>,
        _button: u32,
        pn: &Rc<dyn Node>,
    ) -> bool {
        let Some(ws) = pn.clone().node_into_workspace() else {
            return false;
        };
        self.selector.set(ws);
        spo.revert_to_default(seat);
        true
    }

    fn node_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, node: &Rc<dyn Node>) {
        let mut damage = false;
        let ws = node.clone().node_into_workspace();
        if let Some(ws) = &ws {
            ws.render_highlight.fetch_add(1);
            seat.pointer_cursor().set_known(KnownCursor::Pointer);
            damage = true;
        }
        if let Some(prev) = self.latest.set(ws) {
            prev.render_highlight.fetch_sub(1);
            damage = true;
        }
        if damage {
            seat.state.damage();
        }
    }
}

impl<S: ?Sized> Drop for SelectWorkspaceUsecase<S> {
    fn drop(&mut self) {
        if let Some(prev) = self.latest.take() {
            prev.render_highlight.fetch_sub(1);
            if let Some(seat) = self.seat.upgrade() {
                seat.state.damage();
            }
        }
    }
}

impl SimplePointerOwnerUsecase for WindowManagementUsecase {
    const FIND_TREE_USECASE: FindTreeUsecase = FindTreeUsecase::SelectToplevel;
    const IS_DEFAULT: bool = false;

    fn default_button(
        &self,
        _spo: &SimplePointerOwner<Self>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        pn: &Rc<dyn Node>,
    ) -> bool {
        let Some(tl) = pn.clone().node_into_toplevel() else {
            return false;
        };
        let pos = tl.node_absolute_position();
        let (x, y) = seat.pointer_cursor.position();
        let (x, y) = (x.round_down(), y.round_down());
        let (mut dx, mut dy) = pos.translate(x, y);
        let owner: Rc<dyn PointerOwner> = if button == BTN_LEFT {
            seat.pointer_cursor.set_known(KnownCursor::Move);
            Rc::new(ToplevelGrabPointerOwner {
                tl,
                usecase: MoveToplevelGrabPointerOwner { dx, dy },
            })
        } else if button == BTN_RIGHT {
            let mut top = false;
            let mut right = false;
            let mut bottom = false;
            let mut left = false;
            if dx <= pos.width() / 2 {
                left = true;
            } else {
                right = true;
                dx = pos.width() - dx;
            }
            if dy <= pos.height() / 2 {
                top = true;
            } else {
                bottom = true;
                dy = pos.height() - dy;
            }
            let cursor = match (top, right, bottom, left) {
                (true, true, false, false) => KnownCursor::NeResize,
                (false, true, true, false) => KnownCursor::SeResize,
                (false, false, true, true) => KnownCursor::SwResize,
                (true, false, false, true) => KnownCursor::NwResize,
                _ => KnownCursor::Move,
            };
            seat.pointer_cursor.set_known(cursor);
            Rc::new(ToplevelGrabPointerOwner {
                tl,
                usecase: ResizeToplevelGrabPointerOwner {
                    top,
                    right,
                    bottom,
                    left,
                    dx,
                    dy,
                },
            })
        } else {
            return false;
        };
        seat.pointer_owner.owner.set(owner);
        pn.node_seat_state().add_pointer_grab(seat);
        true
    }

    fn release_grab(&self, seat: &Rc<WlSeatGlobal>) {
        seat.pointer_owner
            .owner
            .set(Rc::new(SimplePointerOwner { usecase: *self }));
        seat.changes.or_assign(CHANGE_CURSOR_MOVED);
    }

    fn disable_window_management(&self, seat: &Rc<WlSeatGlobal>) {
        seat.pointer_owner.set_default_pointer_owner(seat);
        seat.apply_changes();
    }
}

trait WindowManagementGrabUsecase {
    const BUTTON: u32;

    fn apply_changes(
        &self,
        seat: &Rc<WlSeatGlobal>,
        parent: Rc<dyn ContainingNode>,
        tl: &Rc<dyn ToplevelNode>,
    );
}

struct ToplevelGrabPointerOwner<T> {
    tl: Rc<dyn ToplevelNode>,
    usecase: T,
}

impl<T> PointerOwner for ToplevelGrabPointerOwner<T>
where
    T: WindowManagementGrabUsecase,
{
    fn button(&self, seat: &Rc<WlSeatGlobal>, _time_usec: u64, button: u32, state: KeyState) {
        if button != T::BUTTON || state != KeyState::Released {
            return;
        }
        self.tl.node_seat_state().remove_pointer_grab(seat);
        self.grab_node_removed(seat);
    }

    fn axis_node(&self, _seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>> {
        None
    }

    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>) {
        let Some(parent) = self.tl.tl_data().parent.get() else {
            return;
        };
        self.usecase.apply_changes(seat, parent, &self.tl);
    }

    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        seat.pointer_owner.set_default_pointer_owner(seat);
    }

    fn grab_node_removed(&self, seat: &Rc<WlSeatGlobal>) {
        seat.pointer_cursor.set_known(KnownCursor::Default);
        seat.pointer_owner.owner.set(Rc::new(SimplePointerOwner {
            usecase: WindowManagementUsecase,
        }));
        seat.changes.or_assign(CHANGE_CURSOR_MOVED);
        seat.apply_changes();
    }

    fn disable_window_management(&self, seat: &Rc<WlSeatGlobal>) {
        seat.pointer_owner.set_default_pointer_owner(seat);
        seat.apply_changes();
    }
}

struct MoveToplevelGrabPointerOwner {
    dx: i32,
    dy: i32,
}

impl WindowManagementGrabUsecase for MoveToplevelGrabPointerOwner {
    const BUTTON: u32 = BTN_LEFT;

    fn apply_changes(
        &self,
        seat: &Rc<WlSeatGlobal>,
        parent: Rc<dyn ContainingNode>,
        tl: &Rc<dyn ToplevelNode>,
    ) {
        let (x, y) = seat.pointer_cursor.position();
        let (x, y) = (x.round_down() - self.dx, y.round_down() - self.dy);
        parent.cnode_set_child_position(tl.tl_as_node(), x, y);
    }
}

#[derive(Debug)]
struct ResizeToplevelGrabPointerOwner {
    top: bool,
    right: bool,
    bottom: bool,
    left: bool,
    dx: i32,
    dy: i32,
}

impl WindowManagementGrabUsecase for ResizeToplevelGrabPointerOwner {
    const BUTTON: u32 = BTN_RIGHT;

    fn apply_changes(
        &self,
        seat: &Rc<WlSeatGlobal>,
        parent: Rc<dyn ContainingNode>,
        tl: &Rc<dyn ToplevelNode>,
    ) {
        let (x, y) = seat.pointer_cursor.position();
        let (x, y) = (x.round_down(), y.round_down());
        let pos = tl.node_absolute_position();
        let mut x1 = None;
        let mut x2 = None;
        let mut y1 = None;
        let mut y2 = None;
        if self.top {
            let new_v = y - self.dy;
            if new_v != pos.y1() {
                y1 = Some(new_v);
            }
        }
        if self.right {
            let new_v = x + self.dx;
            if new_v != pos.x2() {
                x2 = Some(new_v);
            }
        }
        if self.bottom {
            let new_v = y + self.dy;
            if new_v != pos.y2() {
                y2 = Some(new_v);
            }
        }
        if self.left {
            let new_v = x - self.dx;
            if new_v != pos.x1() {
                x1 = Some(new_v);
            }
        }
        if x1.is_some() || x2.is_some() || y1.is_some() || y2.is_some() {
            parent.cnode_resize_child(tl.tl_as_node(), x1, y1, x2, y2);
        }
    }
}
