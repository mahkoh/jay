use {
    crate::{
        backend::{AxisSource, KeyState, ScrollAxis},
        fixed::Fixed,
        ifs::{
            ipc,
            ipc::{wl_data_device::WlDataDevice, wl_data_source::WlDataSource},
            wl_seat::{wl_pointer::PendingScroll, Dnd, DroppedDnd, WlSeatError, WlSeatGlobal},
            wl_surface::WlSurface,
        },
        tree::{FoundNode, Node, SizedNode},
        utils::{clonecell::CloneCell, smallmap::SmallMap},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct PointerOwnerHolder {
    default: Rc<DefaultPointerOwner>,
    owner: CloneCell<Rc<dyn PointerOwner>>,
    pending_scroll: PendingScroll,
}

impl Default for PointerOwnerHolder {
    fn default() -> Self {
        Self {
            default: Rc::new(DefaultPointerOwner),
            owner: CloneCell::new(Rc::new(DefaultPointerOwner)),
            pending_scroll: Default::default(),
        }
    }
}

impl PointerOwnerHolder {
    pub fn button(&self, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        self.owner.get().button(seat, button, state)
    }

    pub fn axis_source(&self, axis_source: AxisSource) {
        self.pending_scroll.source.set(Some(axis_source as _));
    }

    pub fn axis_discrete(&self, delta: i32, axis: ScrollAxis) {
        self.pending_scroll.discrete[axis as usize].set(Some(delta));
    }

    pub fn axis(&self, delta: Fixed, axis: ScrollAxis) {
        self.pending_scroll.axis[axis as usize].set(Some(delta));
    }

    pub fn axis_stop(&self, axis: ScrollAxis) {
        self.pending_scroll.stop[axis as usize].set(true);
    }

    pub fn frame(&self, seat: &Rc<WlSeatGlobal>) {
        let pending = self.pending_scroll.take();
        if let Some(node) = self.owner.get().axis_node(seat) {
            node.node_axis_event(seat, &pending);
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

    pub fn dnd_target_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().dnd_target_removed(seat);
    }

    pub fn dnd_icon(&self) -> Option<Rc<WlSurface>> {
        self.owner.get().dnd_icon()
    }

    pub fn remove_dnd_icon(&self) {
        self.owner.get().remove_dnd_icon()
    }
}

trait PointerOwner {
    fn button(&self, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState);
    fn axis_node(&self, seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>>;
    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>);
    fn start_drag(
        &self,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
        serial: u32,
    ) -> Result<(), WlSeatError>;
    fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>);
    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>);
    fn dnd_target_removed(&self, seat: &Rc<WlSeatGlobal>);
    fn dnd_icon(&self) -> Option<Rc<WlSurface>>;
    fn remove_dnd_icon(&self);
}

struct DefaultPointerOwner;

struct GrabPointerOwner {
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

impl PointerOwner for DefaultPointerOwner {
    fn button(&self, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        if state != KeyState::Pressed {
            return;
        }
        let pn = match seat.pointer_node() {
            Some(n) => n,
            _ => return,
        };
        let serial = seat.state.next_serial(pn.node_client().as_deref());
        seat.pointer_owner.owner.set(Rc::new(GrabPointerOwner {
            buttons: SmallMap::new_with(button, ()),
            node: pn.clone(),
            serial,
        }));
        pn.node_seat_state().add_pointer_grab(seat);
        pn.node_button(seat, button, state, serial);
    }

    fn axis_node(&self, seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>> {
        seat.pointer_node()
    }

    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>) {
        let (x, y) = seat.pos.get();
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
            .node_find_tree_at(x_int, y_int, &mut found_tree);
        let mut divergence = found_tree.len().min(stack.len());
        for (i, (found, stack)) in found_tree.iter().zip(stack.iter()).enumerate() {
            if found.node.node_id() != stack.node_id() {
                divergence = i;
                break;
            }
        }
        if (stack.len(), found_tree.len()) == (divergence, divergence) {
            if let Some(node) = found_tree.last() {
                node.node.clone().node_pointer_motion(
                    seat,
                    x.apply_fract(node.x),
                    y.apply_fract(node.y),
                );
            }
        } else {
            if let Some(last) = stack.last() {
                last.node_pointer_unfocus(seat);
            }
            for old in stack.drain(divergence..).rev() {
                old.node_leave(seat);
                old.node_seat_state().leave(seat);
            }
            if found_tree.len() == divergence {
                if let Some(node) = found_tree.last() {
                    node.node.clone().node_pointer_motion(
                        seat,
                        x.apply_fract(node.x),
                        y.apply_fract(node.y),
                    );
                }
            } else {
                for new in found_tree.drain(divergence..) {
                    new.node.node_seat_state().enter(seat);
                    new.node.clone().node_pointer_enter(
                        seat,
                        x.apply_fract(new.x),
                        y.apply_fract(new.y),
                    );
                    stack.push(new.node);
                }
            }
            if let Some(node) = stack.last() {
                node.node_pointer_focus(seat);
            }
        }
        found_tree.clear();
    }

    fn start_drag(
        &self,
        _seat: &Rc<WlSeatGlobal>,
        _origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        _icon: Option<Rc<WlSurface>>,
        _serial: u32,
    ) -> Result<(), WlSeatError> {
        if let Some(src) = source {
            src.send_cancelled();
        }
        Ok(())
    }

    fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>) {
        seat.dropped_dnd.borrow_mut().take();
    }

    fn revert_to_default(&self, _seat: &Rc<WlSeatGlobal>) {
        // nothing
    }

    fn dnd_target_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.cancel_dnd(seat);
    }

    fn dnd_icon(&self) -> Option<Rc<WlSurface>> {
        None
    }

    fn remove_dnd_icon(&self) {
        // nothing
    }
}

impl PointerOwner for GrabPointerOwner {
    fn button(&self, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        match state {
            KeyState::Released => {
                self.buttons.remove(&button);
                if self.buttons.is_empty() {
                    self.node.node_seat_state().remove_pointer_grab(seat);
                    seat.tree_changed.trigger();
                    seat.pointer_owner
                        .owner
                        .set(seat.pointer_owner.default.clone());
                }
            }
            KeyState::Pressed => {
                self.buttons.insert(button, ());
            }
        }
        let serial = seat.state.next_serial(self.node.node_client().as_deref());
        self.node.clone().node_button(seat, button, state, serial);
    }

    fn axis_node(&self, _seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>> {
        Some(self.node.clone())
    }

    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>) {
        let (x, y) = seat.pos.get();
        let pos = self.node.node_absolute_position();
        let (x_int, y_int) = pos.translate(x.round_down(), y.round_down());
        self.node
            .clone()
            .node_pointer_motion(seat, x.apply_fract(x_int), y.apply_fract(y_int));
    }

    fn start_drag(
        &self,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        src: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
        serial: u32,
    ) -> Result<(), WlSeatError> {
        let button = match self.buttons.iter().next() {
            Some((b, _)) => b,
            None => return Ok(()),
        };
        if self.buttons.len() != 1 {
            return Ok(());
        }
        if serial != self.serial {
            return Ok(());
        }
        if self.node.node_id() != origin.node_id {
            return Ok(());
        }
        if let Some(icon) = &icon {
            icon.dnd_icons.insert(seat.id(), seat.clone());
        }
        if let Some(new) = &src {
            ipc::attach_seat::<WlDataDevice>(new, seat, ipc::Role::Dnd)?;
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
                node.node_leave(seat);
                node.node_seat_state().leave(seat);
            }
        }
        self.node.node_seat_state().remove_pointer_grab(seat);
        // {
        //     let old = seat.keyboard_node.set(seat.state.root.clone());
        //     old.seat_state().unfocus(seat);
        //     old.unfocus(seat);
        // }
        seat.pointer_owner.owner.set(pointer_owner.clone());
        pointer_owner.apply_changes(seat);
        Ok(())
    }

    fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>) {
        seat.dropped_dnd.borrow_mut().take();
    }

    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.node.node_seat_state().remove_pointer_grab(seat);
        seat.pointer_owner
            .owner
            .set(seat.pointer_owner.default.clone());
    }

    fn dnd_target_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.cancel_dnd(seat)
    }

    fn dnd_icon(&self) -> Option<Rc<WlSurface>> {
        None
    }

    fn remove_dnd_icon(&self) {
        // nothing
    }
}

impl PointerOwner for DndPointerOwner {
    fn button(&self, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        if button != self.button || state != KeyState::Released {
            return;
        }
        let should_drop = match &self.dnd.src {
            None => true,
            Some(s) => s.can_drop(),
        };
        let target = self.target.get();
        if should_drop {
            self.target.get().node_dnd_drop(&self.dnd);
            *seat.dropped_dnd.borrow_mut() = Some(DroppedDnd {
                dnd: self.dnd.clone(),
            });
        }
        target.node_dnd_leave(&self.dnd);
        target.node_seat_state().remove_dnd_target(seat);
        if !should_drop {
            if let Some(src) = &self.dnd.src {
                ipc::detach_seat::<WlDataDevice>(src);
            }
        }
        if let Some(icon) = self.icon.get() {
            icon.dnd_icons.remove(&seat.id());
        }
        seat.pointer_owner
            .owner
            .set(seat.pointer_owner.default.clone());
        seat.tree_changed.trigger();
    }

    fn axis_node(&self, _seat: &Rc<WlSeatGlobal>) -> Option<Rc<dyn Node>> {
        None
    }

    fn apply_changes(&self, seat: &Rc<WlSeatGlobal>) {
        let (x, y) = seat.pos.get();
        let (x_int, y_int) = (x.round_down(), y.round_down());
        let (node, x_int, y_int) = {
            let mut found_tree = seat.found_tree.borrow_mut();
            found_tree.push(FoundNode {
                node: seat.state.root.clone(),
                x: x_int,
                y: y_int,
            });
            seat.state.root.find_tree_at(x_int, y_int, &mut found_tree);
            let FoundNode { node, x, y } = found_tree.pop().unwrap();
            found_tree.clear();
            (node, x, y)
        };
        let (x, y) = (x.apply_fract(x_int), y.apply_fract(y_int));
        let mut target = self.target.get();
        if node.node_id() != target.node_id() {
            target.node_dnd_leave(&self.dnd);
            target.node_seat_state().remove_dnd_target(seat);
            target = node;
            target.node_dnd_enter(
                &self.dnd,
                x,
                y,
                seat.state.next_serial(target.node_client().as_deref()),
            );
            target.node_seat_state().add_dnd_target(seat);
            self.target.set(target);
        } else if (self.pos_x.get(), self.pos_y.get()) != (x, y) {
            node.node_dnd_motion(&self.dnd, x, y);
        }
        self.pos_x.set(x);
        self.pos_y.set(y);
    }

    fn start_drag(
        &self,
        _seat: &Rc<WlSeatGlobal>,
        _origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        _icon: Option<Rc<WlSurface>>,
        _serial: u32,
    ) -> Result<(), WlSeatError> {
        if let Some(src) = source {
            src.send_cancelled();
        }
        Ok(())
    }

    fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>) {
        let target = self.target.get();
        target.node_dnd_leave(&self.dnd);
        target.node_seat_state().remove_dnd_target(seat);
        if let Some(src) = &self.dnd.src {
            ipc::detach_seat::<WlDataDevice>(src);
        }
        if let Some(icon) = self.icon.get() {
            icon.dnd_icons.remove(&seat.id());
        }
        seat.pointer_owner
            .owner
            .set(seat.pointer_owner.default.clone());
        seat.tree_changed.trigger();
    }

    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.cancel_dnd(seat)
    }

    fn dnd_target_removed(&self, seat: &Rc<WlSeatGlobal>) {
        self.target.get().node_dnd_leave(&self.dnd);
        self.target.set(seat.state.root.clone());
        seat.state.tree_changed();
    }

    fn dnd_icon(&self) -> Option<Rc<WlSurface>> {
        self.icon.get()
    }

    fn remove_dnd_icon(&self) {
        self.icon.set(None);
    }
}
