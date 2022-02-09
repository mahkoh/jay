use crate::backend::{KeyState, ScrollAxis};
use crate::fixed::Fixed;
use crate::ifs::ipc;
use crate::ifs::ipc::wl_data_device::WlDataDevice;
use crate::ifs::ipc::wl_data_source::WlDataSource;
use crate::ifs::wl_seat::{Dnd, DroppedDnd, WlSeatError, WlSeatGlobal};
use crate::ifs::wl_surface::{WlSurface};
use crate::tree::{FoundNode, Node};
use crate::utils::clonecell::CloneCell;
use crate::utils::smallmap::SmallMap;
use std::cell::Cell;
use std::rc::Rc;

pub struct PointerOwnerHolder {
    default: Rc<DefaultPointerOwner>,
    owner: CloneCell<Rc<dyn PointerOwner>>,
}

impl Default for PointerOwnerHolder {
    fn default() -> Self {
        Self {
            default: Rc::new(DefaultPointerOwner),
            owner: CloneCell::new(Rc::new(DefaultPointerOwner)),
        }
    }
}

impl PointerOwnerHolder {
    pub fn button(&self, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        self.owner.get().button(seat, button, state)
    }

    pub fn scroll(&self, seat: &Rc<WlSeatGlobal>, delta: i32, axis: ScrollAxis) {
        self.owner.get().scroll(seat, delta, axis)
    }

    pub fn handle_pointer_position(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().handle_pointer_position(seat)
    }

    pub fn start_drag(
        &self,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
    ) -> Result<(), WlSeatError> {
        self.owner.get().start_drag(seat, origin, source, icon)
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
    fn scroll(&self, seat: &Rc<WlSeatGlobal>, delta: i32, axis: ScrollAxis);
    fn handle_pointer_position(&self, seat: &Rc<WlSeatGlobal>);
    fn start_drag(
        &self,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
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
        seat.pointer_owner.owner.set(Rc::new(GrabPointerOwner {
            buttons: SmallMap::new_with(button, ()),
            node: pn.clone(),
        }));
        pn.seat_state().add_pointer_grab(seat);
        pn.button(seat, button, state);
    }

    fn scroll(&self, seat: &Rc<WlSeatGlobal>, delta: i32, axis: ScrollAxis) {
        if let Some(pn) = seat.pointer_node() {
            pn.scroll(seat, delta, axis);
        }
    }

    fn handle_pointer_position(&self, seat: &Rc<WlSeatGlobal>) {
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
        seat.state.root.find_tree_at(x_int, y_int, &mut found_tree);
        let mut divergence = found_tree.len().min(stack.len());
        for (i, (found, stack)) in found_tree.iter().zip(stack.iter()).enumerate() {
            if found.node.id() != stack.id() {
                divergence = i;
                break;
            }
        }
        if (stack.len(), found_tree.len()) == (divergence, divergence) {
            if let Some(node) = found_tree.last() {
                node.node
                    .motion(seat, x.apply_fract(node.x), y.apply_fract(node.y));
            }
        } else {
            if let Some(last) = stack.last() {
                last.pointer_untarget(seat);
            }
            for old in stack.drain(divergence..).rev() {
                old.leave(seat);
                old.seat_state().leave(seat);
            }
            if found_tree.len() == divergence {
                if let Some(node) = found_tree.last() {
                    node.node
                        .clone()
                        .motion(seat, x.apply_fract(node.x), y.apply_fract(node.y));
                }
            } else {
                for new in found_tree.drain(divergence..) {
                    new.node.seat_state().enter(seat);
                    new.node
                        .clone()
                        .enter(seat, x.apply_fract(new.x), y.apply_fract(new.y));
                    stack.push(new.node);
                }
            }
            if let Some(node) = stack.last() {
                node.pointer_target(seat);
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
                    self.node.seat_state().remove_pointer_grab(seat);
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
        self.node.clone().button(seat, button, state);
    }

    fn scroll(&self, seat: &Rc<WlSeatGlobal>, delta: i32, axis: ScrollAxis) {
        self.node.scroll(seat, delta, axis);
    }

    fn handle_pointer_position(&self, seat: &Rc<WlSeatGlobal>) {
        let (x, y) = seat.pos.get();
        let pos = self.node.absolute_position();
        let (x_int, y_int) = pos.translate(x.round_down(), y.round_down());
        self.node
            .motion(seat, x.apply_fract(x_int), y.apply_fract(y_int));
    }

    fn start_drag(
        &self,
        seat: &Rc<WlSeatGlobal>,
        origin: &Rc<WlSurface>,
        src: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
    ) -> Result<(), WlSeatError> {
        let button = match self.buttons.iter().next() {
            Some((b, _)) => b,
            None => return Ok(()),
        };
        if self.buttons.len() != 1 {
            return Ok(());
        }
        if self.node.id() != origin.node_id {
            return Ok(());
        }
        if let Some(icon) = &icon {
            icon.dnd_icons.insert(seat.id(), seat.clone());
        }
        if let Some(new) = &src {
            ipc::attach_seat::<WlDataDevice>(&new, seat, ipc::Role::Dnd)?;
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
                node.leave(seat);
                node.seat_state().leave(seat);
            }
        }
        self.node.seat_state().remove_pointer_grab(seat);
        // {
        //     let old = seat.keyboard_node.set(seat.state.root.clone());
        //     old.seat_state().unfocus(seat);
        //     old.unfocus(seat);
        // }
        seat.pointer_owner.owner.set(pointer_owner.clone());
        pointer_owner.handle_pointer_position(seat);
        Ok(())
    }

    fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>) {
        seat.dropped_dnd.borrow_mut().take();
    }

    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.node.seat_state().remove_pointer_grab(seat);
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
            self.target.get().dnd_drop(&self.dnd);
            *seat.dropped_dnd.borrow_mut() = Some(DroppedDnd {
                dnd: self.dnd.clone(),
            });
        }
        target.dnd_leave(&self.dnd);
        target.seat_state().remove_dnd_target(seat);
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

    fn scroll(&self, _seat: &Rc<WlSeatGlobal>, _delta: i32, _axis: ScrollAxis) {
        // nothing
    }

    fn handle_pointer_position(&self, seat: &Rc<WlSeatGlobal>) {
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
        if node.id() != target.id() {
            target.dnd_leave(&self.dnd);
            target.seat_state().remove_dnd_target(seat);
            target = node;
            target.dnd_enter(&self.dnd, x, y);
            target.seat_state().add_dnd_target(seat);
            self.target.set(target);
        } else if (self.pos_x.get(), self.pos_y.get()) != (x, y) {
            node.dnd_motion(&self.dnd, x, y);
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
    ) -> Result<(), WlSeatError> {
        if let Some(src) = source {
            src.send_cancelled();
        }
        Ok(())
    }

    fn cancel_dnd(&self, seat: &Rc<WlSeatGlobal>) {
        let target = self.target.get();
        target.dnd_leave(&self.dnd);
        target.seat_state().remove_dnd_target(seat);
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
        self.target.get().dnd_leave(&self.dnd);
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
