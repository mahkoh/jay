use crate::backend::{InputEvent, KeyState, OutputId, ScrollAxis};
use crate::client::{Client, ClientId};
use crate::fixed::Fixed;
use crate::ifs::ipc;
use crate::ifs::ipc::wl_data_device::WlDataDevice;
use crate::ifs::ipc::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use crate::ifs::wl_seat::wl_keyboard::WlKeyboard;
use crate::ifs::wl_seat::wl_pointer::{WlPointer, POINTER_FRAME_SINCE_VERSION};
use crate::ifs::wl_seat::{wl_keyboard, wl_pointer, Dnd, SeatId, WlSeat, WlSeatGlobal};
use crate::ifs::wl_surface::xdg_surface::xdg_popup::XdgPopup;
use crate::ifs::wl_surface::WlSurface;
use crate::object::ObjectId;
use crate::tree::toplevel::ToplevelNode;
use crate::tree::{FloatNode, Node, OutputNode};
use crate::utils::clonecell::CloneCell;
use crate::utils::smallmap::SmallMap;
use crate::wire::WlDataOfferId;
use crate::xkbcommon::{ModifierState, XKB_KEY_DOWN, XKB_KEY_UP};
use jay_config::keyboard::mods::Modifiers;
use jay_config::keyboard::syms::KeySym;
use jay_config::keyboard::ModifiedKeySym;
use smallvec::SmallVec;
use std::ops::Deref;
use std::rc::Rc;

#[derive(Default)]
pub struct NodeSeatState {
    pointer_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    kb_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    grabs: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    dnd_targets: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
}

impl NodeSeatState {
    pub(super) fn enter(&self, seat: &Rc<WlSeatGlobal>) {
        self.pointer_foci.insert(seat.id, seat.clone());
    }

    pub(super) fn leave(&self, seat: &WlSeatGlobal) {
        self.pointer_foci.remove(&seat.id);
    }

    pub(super) fn focus(&self, seat: &Rc<WlSeatGlobal>) -> bool {
        self.kb_foci.insert(seat.id, seat.clone());
        self.kb_foci.len() == 1
    }

    pub(super) fn unfocus(&self, seat: &WlSeatGlobal) -> bool {
        self.kb_foci.remove(&seat.id);
        self.kb_foci.len() == 0
    }

    pub(super) fn add_pointer_grab(&self, seat: &Rc<WlSeatGlobal>) {
        self.grabs.insert(seat.id, seat.clone());
    }

    pub(super) fn remove_pointer_grab(&self, seat: &WlSeatGlobal) {
        self.grabs.remove(&seat.id);
    }

    pub(super) fn add_dnd_target(&self, seat: &Rc<WlSeatGlobal>) {
        self.dnd_targets.insert(seat.id, seat.clone());
    }

    pub(super) fn remove_dnd_target(&self, seat: &WlSeatGlobal) {
        self.dnd_targets.remove(&seat.id);
    }

    pub fn is_active(&self) -> bool {
        self.kb_foci.len() > 0
    }

    pub fn release_kb_grab(&self) {
        for (_, seat) in &self.kb_foci {
            seat.ungrab_kb();
        }
    }

    pub fn release_kb_focus(&self) {
        while let Some((_, seat)) = self.kb_foci.pop() {
            seat.ungrab_kb();
            seat.keyboard_node.set(seat.state.root.clone());
            if let Some(tl) = seat.toplevel_focus_history.last() {
                seat.focus_node(tl.focus_surface(seat.id));
            }
        }
    }

    pub fn destroy_node(&self, node: &dyn Node) {
        while let Some((_, seat)) = self.grabs.pop() {
            seat.pointer_owner.revert_to_default(&seat);
        }
        let node_id = node.id();
        while let Some((_, seat)) = self.dnd_targets.pop() {
            seat.pointer_owner.dnd_target_removed(&seat);
        }
        while let Some((_, seat)) = self.pointer_foci.pop() {
            let mut ps = seat.pointer_stack.borrow_mut();
            while let Some(last) = ps.pop() {
                if last.id() == node_id {
                    break;
                }
                last.seat_state().leave(&seat);
                last.leave(&seat);
            }
            seat.state.tree_changed();
        }
        self.release_kb_focus();
    }
}

impl WlSeatGlobal {
    pub fn event(self: &Rc<Self>, event: InputEvent) {
        match event {
            InputEvent::Key(k, s) => self.key_event(k, s),
            InputEvent::OutputPosition(o, x, y) => self.output_position_event(o, x, y),
            InputEvent::Motion(dx, dy) => self.motion_event(dx, dy),
            InputEvent::Button(b, s) => self.pointer_owner.button(self, b, s),
            InputEvent::Scroll(d, a) => self.pointer_owner.scroll(self, d, a),
        }
    }

    fn output_position_event(self: &Rc<Self>, output: OutputId, mut x: Fixed, mut y: Fixed) {
        let output = match self.state.outputs.get(&output) {
            Some(o) => o,
            _ => return,
        };
        let pos = output.position();
        x += Fixed::from_int(pos.x1());
        y += Fixed::from_int(pos.y1());
        self.set_new_position(x, y);
    }

    fn motion_event(self: &Rc<Self>, dx: Fixed, dy: Fixed) {
        let (x, y) = self.pos.get();
        self.set_new_position(x + dx, y + dy);
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
        let mut shortcuts = SmallVec::<[_; 1]>::new();
        let new_mods;
        {
            let mut kb_state = self.kb_state.borrow_mut();
            if state == wl_keyboard::PRESSED {
                let old_mods = kb_state.mods();
                let keysyms = kb_state.unmodified_keysyms(key);
                for &sym in keysyms {
                    if let Some(mods) = self.shortcuts.get(&(old_mods.mods_effective, sym)) {
                        shortcuts.push(ModifiedKeySym {
                            mods,
                            sym: KeySym(sym),
                        });
                    }
                }
            }
            new_mods = kb_state.update(key, xkb_dir);
        }
        let node = self.keyboard_node.get();
        if shortcuts.is_empty() {
            node.key(self, key, state);
        } else if let Some(config) = self.state.config.get() {
            for shortcut in shortcuts {
                config.invoke_shortcut(self.id(), &shortcut);
            }
        }
        if let Some(mods) = new_mods {
            node.mods(self, mods);
        }
    }
}

impl WlSeatGlobal {
    pub(super) fn pointer_node(&self) -> Option<Rc<dyn Node>> {
        self.pointer_stack.borrow().last().cloned()
    }

    pub fn last_tiled_keyboard_toplevel(&self, new: &dyn Node) -> Option<Rc<dyn ToplevelNode>> {
        let is_container = new.is_container();
        for tl in self.toplevel_focus_history.rev_iter() {
            let parent_is_float = match tl.parent() {
                Some(pn) => pn.is_float(),
                _ => false,
            };
            if !parent_is_float && (!is_container || !tl.as_node().is_contained_in(new.id())) {
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

    pub fn focus_toplevel(self: &Rc<Self>, n: Rc<dyn ToplevelNode>) {
        let node = self.toplevel_focus_history.add_last(n.clone());
        n.data().toplevel_history.insert(self.id, node);
        self.focus_node(n.focus_surface(self.id));
    }

    fn ungrab_kb(self: &Rc<Self>) {
        self.kb_owner.ungrab(self);
    }

    pub fn grab(self: &Rc<Self>, node: Rc<dyn Node>) {
        self.kb_owner.grab(self, node);
    }

    pub fn focus_node(self: &Rc<Self>, node: Rc<dyn Node>) {
        self.kb_owner.set_kb_node(self, node);
    }

    fn offer_selection<T: ipc::Vtable>(
        &self,
        field: &CloneCell<Option<Rc<T::Source>>>,
        client: &Rc<Client>,
    ) {
        match field.get() {
            Some(sel) => ipc::offer_source_to::<T>(&sel, client),
            None => T::for_each_device(self, client.id, |dd| {
                T::send_selection(dd, ObjectId::NONE.into());
            }),
        }
    }

    fn for_each_seat<C>(&self, ver: u32, client: ClientId, mut f: C)
    where
        C: FnMut(&Rc<WlSeat>),
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

    pub fn for_each_data_device<C>(&self, ver: u32, client: ClientId, mut f: C)
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

    pub fn for_each_primary_selection_device<C>(&self, ver: u32, client: ClientId, mut f: C)
    where
        C: FnMut(&Rc<ZwpPrimarySelectionDeviceV1>),
    {
        let dd = self.primary_selection_devices.borrow_mut();
        if let Some(dd) = dd.get(&client) {
            for dd in dd.values() {
                if dd.manager.version >= ver {
                    f(dd);
                }
            }
        }
    }

    fn surface_pointer_frame(&self, surface: &WlSurface) {
        self.surface_pointer_event(POINTER_FRAME_SINCE_VERSION, surface, |p| p.send_frame());
    }

    fn surface_pointer_event<F>(&self, ver: u32, surface: &WlSurface, mut f: F)
    where
        F: FnMut(&Rc<WlPointer>),
    {
        let client = &surface.client;
        self.for_each_pointer(ver, client.id, |p| {
            f(p);
        });
        client.flush();
    }

    fn surface_kb_event<F>(&self, ver: u32, surface: &WlSurface, mut f: F)
    where
        F: FnMut(&Rc<WlKeyboard>),
    {
        let client = &surface.client;
        self.for_each_kb(ver, client.id, |p| {
            f(p);
        });
        client.flush();
    }

    fn set_new_position(self: &Rc<Self>, x: Fixed, y: Fixed) {
        self.pos.set((x, y));
        self.handle_new_position(true);
    }

    pub fn add_shortcut(&self, mods: Modifiers, keysym: KeySym) {
        self.shortcuts.set((mods.0, keysym.0), mods);
    }

    pub fn remove_shortcut(&self, mods: Modifiers, keysym: KeySym) {
        self.shortcuts.remove(&(mods.0, keysym.0));
    }

    pub fn trigger_tree_changed(&self) {
        self.tree_changed.trigger();
    }

    pub(super) fn tree_changed(self: &Rc<Self>) {
        self.handle_new_position(false);
    }

    fn handle_new_position(self: &Rc<Self>, pos_changed: bool) {
        let (x, y) = self.pos.get();
        if pos_changed {
            if let Some(cursor) = self.cursor.get() {
                cursor.set_position(x.round_down(), y.round_down());
            }
        }
        self.pointer_owner.handle_pointer_position(self);
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
        self.surface_pointer_event(0, surface, |p| p.send_button(serial, 0, button, state));
        self.surface_pointer_frame(surface);
        if pressed && surface.accepts_kb_focus() {
            self.focus_node(surface.clone());
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
        self.surface_pointer_event(0, surface, |p| p.send_axis(0, axis, Fixed::from_int(delta)));
        self.surface_pointer_frame(surface);
    }
}

// Motion callbacks
impl WlSeatGlobal {
    pub fn motion_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        self.surface_pointer_event(0, n, |p| p.send_motion(0, x, y));
        self.surface_pointer_frame(n);
    }
}

// Enter callbacks
impl WlSeatGlobal {
    pub fn enter_toplevel(self: &Rc<Self>, n: Rc<dyn ToplevelNode>) {
        self.focus_toplevel(n);
    }

    pub fn enter_popup(self: &Rc<Self>, _n: &Rc<XdgPopup>) {
        // self.focus_xdg_surface(&n.xdg);
    }

    pub fn enter_output(self: &Rc<Self>, output: &Rc<OutputNode>) {
        self.output.set(output.clone());
    }

    pub fn enter_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        let serial = self.serial.fetch_add(1);
        self.surface_pointer_event(0, n, |p| p.send_enter(serial, n.id, x, y));
        self.surface_pointer_frame(n);
    }
}

// Leave callbacks
impl WlSeatGlobal {
    pub fn leave_surface(&self, n: &WlSurface) {
        let serial = self.serial.fetch_add(1);
        self.surface_pointer_event(0, n, |p| p.send_leave(serial, n.id));
        self.surface_pointer_frame(n);
    }

    pub fn leave_output(&self) {
        self.output.set(self.state.dummy_output.get().unwrap());
    }
}

// Unfocus callbacks
impl WlSeatGlobal {
    pub fn unfocus_surface(&self, surface: &WlSurface) {
        self.surface_kb_event(0, surface, |k| k.send_leave(0, surface.id))
    }
}

// Focus callbacks
impl WlSeatGlobal {
    pub fn focus_surface(&self, surface: &WlSurface) {
        let pressed_keys: Vec<_> = self.pressed_keys.borrow().iter().copied().collect();
        let serial = self.serial.fetch_add(1);
        self.surface_kb_event(0, surface, |k| {
            k.send_enter(serial, surface.id, &pressed_keys)
        });
        let ModifierState {
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
            ..
        } = self.kb_state.borrow().mods();
        let serial = self.serial.fetch_add(1);
        self.surface_kb_event(0, surface, |k| {
            k.send_modifiers(serial, mods_depressed, mods_latched, mods_locked, group)
        });

        if self.keyboard_node.get().client_id() != Some(surface.client.id) {
            self.offer_selection::<WlDataDevice>(&self.selection, &surface.client);
            self.offer_selection::<ZwpPrimarySelectionDeviceV1>(
                &self.primary_selection,
                &surface.client,
            );
        }
    }
}

// Key callbacks
impl WlSeatGlobal {
    pub fn key_surface(&self, surface: &WlSurface, key: u32, state: u32) {
        let serial = self.serial.fetch_add(1);
        self.surface_kb_event(0, surface, |k| k.send_key(serial, 0, key, state));
    }
}

// Modifiers callbacks
impl WlSeatGlobal {
    pub fn mods_surface(&self, surface: &WlSurface, mods: ModifierState) {
        let serial = self.serial.fetch_add(1);
        self.surface_kb_event(0, surface, |k| {
            k.send_modifiers(
                serial,
                mods.mods_depressed,
                mods.mods_latched,
                mods.mods_locked,
                mods.group,
            )
        });
    }
}

// Dnd callbacks
impl WlSeatGlobal {
    pub fn dnd_surface_leave(&self, surface: &WlSurface, dnd: &Dnd) {
        if dnd.src.is_some() || surface.client.id == dnd.client.id {
            self.for_each_data_device(0, surface.client.id, |dd| {
                dd.send_leave();
            })
        }
        if let Some(src) = &dnd.src {
            src.on_leave();
        }
        surface.client.flush();
    }

    pub fn dnd_surface_drop(&self, surface: &WlSurface, dnd: &Dnd) {
        if dnd.src.is_some() || surface.client.id == dnd.client.id {
            self.for_each_data_device(0, surface.client.id, |dd| {
                dd.send_drop();
            })
        }
        if let Some(src) = &dnd.src {
            src.on_drop();
        }
        surface.client.flush();
    }

    pub fn dnd_surface_enter(&self, surface: &WlSurface, dnd: &Dnd, x: Fixed, y: Fixed) {
        if let Some(src) = &dnd.src {
            ipc::offer_source_to::<WlDataDevice>(src, &surface.client);
            src.for_each_data_offer(|offer| {
                offer.device.send_enter(surface.id, x, y, offer.id);
                offer.send_source_actions();
            })
        } else if surface.client.id == dnd.client.id {
            self.for_each_data_device(0, dnd.client.id, |dd| {
                dd.send_enter(surface.id, x, y, WlDataOfferId::NONE);
            })
        }
        surface.client.flush();
    }

    pub fn dnd_surface_motion(&self, surface: &WlSurface, dnd: &Dnd, x: Fixed, y: Fixed) {
        if dnd.src.is_some() || surface.client.id == dnd.client.id {
            self.for_each_data_device(0, surface.client.id, |dd| {
                dd.send_motion(x, y);
            })
        }
        surface.client.flush();
    }
}
