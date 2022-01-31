use std::ops::Deref;
use std::rc::Rc;
use crate::backend::{KeyState, OutputId, ScrollAxis, SeatEvent, SeatId};
use crate::client::{ClientId, DynEventFormatter};
use crate::fixed::Fixed;
use crate::ifs::wl_data_device::WlDataDevice;
use crate::ifs::wl_data_offer::WlDataOfferId;
use crate::ifs::wl_seat::{wl_keyboard, wl_pointer, WlSeatGlobal, WlSeatObj};
use crate::ifs::wl_seat::wl_keyboard::WlKeyboard;
use crate::ifs::wl_seat::wl_pointer::{POINTER_FRAME_SINCE_VERSION, WlPointer};
use crate::ifs::wl_surface::WlSurface;
use crate::ifs::wl_surface::xdg_surface::xdg_popup::XdgPopup;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::tree::{FloatNode, FoundNode, Node};
use crate::utils::smallmap::SmallMap;
use crate::xkbcommon::{ModifierState, XKB_KEY_DOWN, XKB_KEY_UP};

#[derive(Default)]
pub struct NodeSeatState {
    pointer_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    kb_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
}

impl NodeSeatState {
    fn enter(&self, seat: &Rc<WlSeatGlobal>) {
        self.pointer_foci.insert(seat.seat.id(), seat.clone());
    }

    fn leave(&self, seat: &WlSeatGlobal) {
        self.pointer_foci.remove(&seat.seat.id());
    }

    fn focus(&self, seat: &Rc<WlSeatGlobal>) -> bool {
        self.kb_foci.insert(seat.seat.id(), seat.clone());
        self.kb_foci.len() == 1
    }

    fn unfocus(&self, seat: &WlSeatGlobal) -> bool {
        self.kb_foci.remove(&seat.seat.id());
        self.kb_foci.len() == 0
    }

    pub fn is_active(&self) -> bool {
        self.kb_foci.len() > 0
    }

    pub fn destroy_node(&self, node: &dyn Node) {
        let node_id = node.id();
        while let Some((_, seat)) = self.pointer_foci.pop() {
            let mut ps = seat.pointer_stack.borrow_mut();
            while let Some(last) = ps.pop() {
                if last.id() == node_id {
                    break;
                }
            }
            seat.state.tree_changed();
        }
        while let Some((_, seat)) = self.kb_foci.pop() {
            seat.keyboard_node.set(seat.state.root.clone());
            if let Some(tl) = seat.toplevel_focus_history.last() {
                seat.focus_xdg_surface(&tl.xdg);
            }
        }
    }
}

impl WlSeatGlobal {
    pub fn event(self: &Rc<Self>, event: SeatEvent) {
        match event {
            SeatEvent::OutputPosition(o, x, y) => self.output_position_event(o, x, y),
            SeatEvent::Motion(dx, dy) => self.motion_event(dx, dy),
            SeatEvent::Button(b, s) => self.button_event(b, s),
            SeatEvent::Scroll(d, a) => self.scroll_event(d, a),
            SeatEvent::Key(k, s) => self.key_event(k, s),
        }
    }

    fn output_position_event(self: &Rc<Self>, output: OutputId, mut x: Fixed, mut y: Fixed) {
        let output = match self.state.outputs.get(&output) {
            Some(o) => o,
            _ => return,
        };
        x += Fixed::from_int(output.x.get());
        y += Fixed::from_int(output.y.get());
        self.set_new_position(x, y);
    }

    fn motion_event(self: &Rc<Self>, dx: Fixed, dy: Fixed) {
        let (x, y) = self.pos.get();
        self.set_new_position(x + dx, y + dy);
    }

    fn button_event(self: &Rc<Self>, button: u32, state: KeyState) {
        if state == KeyState::Released {
            self.move_.set(false);
        }
        if let Some(node) = self.pointer_node() {
            node.button(self, button, state);
        }
    }

    fn scroll_event(&self, delta: i32, axis: ScrollAxis) {
        if let Some(node) = self.pointer_node() {
            node.scroll(self, delta, axis);
        }
    }

    fn key_event(&self, key: u32, state: KeyState) {
        let (state, xkb_dir) = {
            let mut pk = self.pressed_keys.borrow_mut();
            match state {
                KeyState::Released => {
                    if !pk.remove(&key) {
                        return;
                    }
                    (wl_keyboard::RELEASED, XKB_KEY_UP)
                }
                KeyState::Pressed => {
                    if !pk.insert(key) {
                        return;
                    }
                    (wl_keyboard::PRESSED, XKB_KEY_DOWN)
                }
            }
        };
        let mods = self.kb_state.borrow_mut().update(key, xkb_dir);
        let node = self.keyboard_node.get();
        node.key(self, key, state, mods);
    }
}

impl WlSeatGlobal {
    fn pointer_node(&self) -> Option<Rc<dyn Node>> {
        self.pointer_stack.borrow().last().cloned()
    }

    pub fn last_tiled_keyboard_toplevel(&self) -> Option<Rc<XdgToplevel>> {
        for tl in self.toplevel_focus_history.rev_iter() {
            if !tl.parent_is_float() {
                return Some(tl.deref().clone());
            }
        }
        None
    }

    pub fn move_(&self, node: &Rc<FloatNode>) {
        self.move_.set(true);
        self.move_start_pos.set(self.pos.get());
        let ex = node.position.get();
        self.extents_start_pos.set((ex.x1(), ex.y1()));
    }

    pub fn focus_toplevel(self: &Rc<Self>, n: &Rc<XdgToplevel>) {
        let node = self.toplevel_focus_history.add_last(n.clone());
        n.toplevel_history.insert(self.id(), node);
        self.focus_xdg_surface(&n.xdg);
    }

    fn focus_xdg_surface(self: &Rc<Self>, xdg: &Rc<XdgSurface>) {
        self.focus_surface(&xdg.focus_surface(self));
    }

    fn focus_surface(self: &Rc<Self>, surface: &Rc<WlSurface>) {
        let old = self.keyboard_node.get();
        if old.id() == surface.node_id {
            return;
        }
        old.unfocus(self);
        if old.seat_state().unfocus(self) {
            old.active_changed(false);
        }

        if surface.seat_state().focus(self) {
            surface.active_changed(true);
        }
        surface.clone().focus(self);
        self.keyboard_node.set(surface.clone());

        let pressed_keys: Vec<_> = self.pressed_keys.borrow().iter().copied().collect();
        let serial = self.serial.fetch_add(1);
        self.surface_kb_event(0, &surface, |k| {
            k.enter(serial, surface.id, pressed_keys.clone())
        });
        let ModifierState {
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        } = self.kb_state.borrow().mods();
        let serial = self.serial.fetch_add(1);
        self.surface_kb_event(0, &surface, |k| {
            k.modifiers(serial, mods_depressed, mods_latched, mods_locked, group)
        });

        self.surface_data_device_event(0, &surface, |dd| dd.selection(WlDataOfferId::NONE));
    }

    fn for_each_seat<C>(&self, ver: u32, client: ClientId, mut f: C)
        where
            C: FnMut(&Rc<WlSeatObj>),
    {
        let bindings = self.bindings.borrow();
        if let Some(hm) = bindings.get(&client) {
            for seat in hm.values() {
                if seat.version >= ver {
                    f(seat);
                }
            }
        }
    }

    fn for_each_pointer<C>(&self, ver: u32, client: ClientId, mut f: C)
        where
            C: FnMut(&Rc<WlPointer>),
    {
        self.for_each_seat(ver, client, |seat| {
            let pointers = seat.pointers.lock();
            for pointer in pointers.values() {
                f(pointer);
            }
        })
    }

    fn for_each_kb<C>(&self, ver: u32, client: ClientId, mut f: C)
        where
            C: FnMut(&Rc<WlKeyboard>),
    {
        self.for_each_seat(ver, client, |seat| {
            let keyboards = seat.keyboards.lock();
            for keyboard in keyboards.values() {
                f(keyboard);
            }
        })
    }

    fn for_each_data_device<C>(&self, ver: u32, client: ClientId, mut f: C)
        where
            C: FnMut(&Rc<WlDataDevice>),
    {
        let dd = self.data_devices.borrow_mut();
        if let Some(dd) = dd.get(&client) {
            for dd in dd.values() {
                if dd.manager.version >= ver {
                    f(dd);
                }
            }
        }
    }

    fn surface_pointer_frame(&self, surface: &WlSurface) {
        self.surface_pointer_event(POINTER_FRAME_SINCE_VERSION, surface, |p| p.frame());
    }

    fn surface_pointer_event<F>(&self, ver: u32, surface: &WlSurface, mut f: F)
        where
            F: FnMut(&Rc<WlPointer>) -> DynEventFormatter,
    {
        let client = &surface.client;
        self.for_each_pointer(ver, client.id, |p| {
            client.event(f(p));
        });
        client.flush();
    }

    fn surface_kb_event<F>(&self, ver: u32, surface: &WlSurface, mut f: F)
        where
            F: FnMut(&Rc<WlKeyboard>) -> DynEventFormatter,
    {
        let client = &surface.client;
        self.for_each_kb(ver, client.id, |p| {
            client.event(f(p));
        });
        client.flush();
    }

    fn surface_data_device_event<F>(&self, ver: u32, surface: &WlSurface, mut f: F)
        where
            F: FnMut(&Rc<WlDataDevice>) -> DynEventFormatter,
    {
        let client = &surface.client;
        self.for_each_data_device(ver, client.id, |p| {
            client.event(f(p));
        });
        client.flush();
    }

    fn set_new_position(self: &Rc<Self>, x: Fixed, y: Fixed) {
        self.pos.set((x, y));
        self.handle_new_position(true);
    }

    pub fn tree_changed(self: &Rc<Self>) {
        self.handle_new_position(false);
    }

    fn handle_new_position(self: &Rc<Self>, changed: bool) {
        let (x, y) = self.pos.get();
        if changed {
            if let Some(cursor) = self.cursor.get() {
                cursor.set_position(x.round_down(), y.round_down());
            }
        }
        let mut found_tree = self.found_tree.borrow_mut();
        let mut stack = self.pointer_stack.borrow_mut();
        // if self.move_.get() {
        //     for node in stack.iter().rev() {
        //         if let NodeKind::Toplevel(tn) = node.clone().into_kind() {
        //             let (move_start_x, move_start_y) = self.move_start_pos.get();
        //             let (move_start_ex, move_start_ey) = self.extents_start_pos.get();
        //             let mut ex = tn.common.extents.get();
        //             ex.x = (x - move_start_x).round_down() + move_start_ex;
        //             ex.y = (y - move_start_y).round_down() + move_start_ey;
        //             tn.common.extents.set(ex);
        //         }
        //     }
        //     return;
        // }
        let x_int = x.round_down();
        let y_int = y.round_down();
        found_tree.push(FoundNode {
            node: self.state.root.clone(),
            x: x_int,
            y: y_int,
        });
        self.state.root.find_tree_at(x_int, y_int, &mut found_tree);
        let mut divergence = found_tree.len().min(stack.len());
        for (i, (found, stack)) in found_tree.iter().zip(stack.iter()).enumerate() {
            if found.node.id() != stack.id() {
                divergence = i;
                break;
            }
        }
        if (stack.len(), found_tree.len()) == (divergence, divergence) {
            if changed {
                if let Some(node) = found_tree.last() {
                    node.node.motion(self, x.apply_fract(node.x), y.apply_fract(node.y));
                }
            }
        } else {
            for old in stack.drain(divergence..).rev() {
                old.leave(self);
                old.seat_state().leave(self);
            }
            for new in found_tree.drain(divergence..) {
                new.node.seat_state().enter(self);
                new.node.clone().enter(self, x.apply_fract(new.x), y.apply_fract(new.y));
                stack.push(new.node);
            }
        }
        found_tree.clear();
    }
}

// Button callbacks
impl WlSeatGlobal {
    pub fn button_surface(self: &Rc<Self>, surface: &Rc<WlSurface>, button: u32, state: KeyState) {
        let (state, pressed) = match state {
            KeyState::Released => (wl_pointer::RELEASED, false),
            KeyState::Pressed => (wl_pointer::PRESSED, true),
        };
        let serial = self.serial.fetch_add(1);
        self.surface_pointer_event(0, surface, |p| p.button(serial, 0, button, state));
        self.surface_pointer_frame(surface);
        if pressed && surface.belongs_to_toplevel() {
            self.focus_surface(surface);
        }
    }
}

// Scroll callbacks
impl WlSeatGlobal {
    pub fn scroll_surface(&self, surface: &WlSurface, delta: i32, axis: ScrollAxis) {
        let axis = match axis {
            ScrollAxis::Horizontal => wl_pointer::HORIZONTAL_SCROLL,
            ScrollAxis::Vertical => wl_pointer::VERTICAL_SCROLL,
        };
        self.surface_pointer_event(0, surface, |p| p.axis(0, axis, Fixed::from_int(delta)));
        self.surface_pointer_frame(surface);
    }
}

// Motion callbacks
impl WlSeatGlobal {
    pub fn motion_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        self.surface_pointer_event(0, n, |p| p.motion(0, x, y));
        self.surface_pointer_frame(n);
    }
}

// Enter callbacks
impl WlSeatGlobal {
    pub fn enter_toplevel(self: &Rc<Self>, n: &Rc<XdgToplevel>) {
        self.focus_toplevel(n);
    }

    pub fn enter_popup(self: &Rc<Self>, n: &Rc<XdgPopup>) {
        // self.focus_xdg_surface(&n.xdg);
    }

    pub fn enter_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        let serial = self.serial.fetch_add(1);
        self.surface_pointer_event(0, n, |p| p.enter(serial, n.id, x, y));
        self.surface_pointer_frame(n);
    }
}

// Leave callbacks
impl WlSeatGlobal {
    pub fn leave_surface(&self, n: &WlSurface) {
        let serial = self.serial.fetch_add(1);
        self.surface_pointer_event(0, n, |p| p.leave(serial, n.id));
        self.surface_pointer_frame(n);
    }
}

// Unfocus callbacks
impl WlSeatGlobal {
    pub fn unfocus_surface(&self, surface: &WlSurface) {
        self.surface_kb_event(0, surface, |k| k.leave(0, surface.id))
    }
}

// Key callbacks
impl WlSeatGlobal {
    pub fn key_surface(&self, surface: &WlSurface, key: u32, state: u32, mods: Option<ModifierState>) {
        let serial = self.serial.fetch_add(1);
        self.surface_kb_event(0, surface, |k| k.key(serial, 0, key, state));
        let serial = self.serial.fetch_add(1);
        if let Some(mods) = mods {
            self.surface_kb_event(0, surface, |k| {
                k.modifiers(
                    serial,
                    mods.mods_depressed,
                    mods.mods_latched,
                    mods.mods_locked,
                    mods.group,
                )
            });
        }
    }
}
