use {
    crate::{
        backend::{ConnectorId, InputEvent, KeyState},
        client::{Client, ClientId},
        fixed::Fixed,
        ifs::{
            ipc,
            ipc::{
                wl_data_device::{ClipboardIpc, WlDataDevice},
                zwp_primary_selection_device_v1::{
                    PrimarySelectionIpc, ZwpPrimarySelectionDeviceV1,
                },
            },
            wl_seat::{
                wl_keyboard::{self, WlKeyboard},
                wl_pointer::{
                    self, PendingScroll, WlPointer, AXIS_DISCRETE_SINCE_VERSION,
                    AXIS_SOURCE_SINCE_VERSION, AXIS_STOP_SINCE_VERSION,
                    POINTER_FRAME_SINCE_VERSION, WHEEL_TILT, WHEEL_TILT_SINCE_VERSION,
                },
                zwp_relative_pointer_v1::ZwpRelativePointerV1,
                Dnd, SeatId, WlSeat, WlSeatGlobal, CHANGE_CURSOR_MOVED,
            },
            wl_surface::{xdg_surface::xdg_popup::XdgPopup, WlSurface},
        },
        tree::{Direction, FloatNode, Node, ToplevelNode},
        utils::{bitflags::BitflagsExt, clonecell::CloneCell, smallmap::SmallMap},
        wire::WlDataOfferId,
        xkbcommon::{ModifierState, XKB_KEY_DOWN, XKB_KEY_UP},
    },
    jay_config::keyboard::{mods::Modifiers, syms::KeySym, ModifiedKeySym},
    smallvec::SmallVec,
    std::rc::Rc,
};

#[derive(Default)]
pub struct NodeSeatState {
    pointer_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    kb_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    pointer_grabs: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
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
        self.pointer_grabs.insert(seat.id, seat.clone());
    }

    pub(super) fn remove_pointer_grab(&self, seat: &WlSeatGlobal) {
        self.pointer_grabs.remove(&seat.id);
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
        self.release_kb_focus2(true);
    }

    fn release_kb_focus2(&self, focus_last: bool) {
        self.release_kb_grab();
        while let Some((_, seat)) = self.kb_foci.pop() {
            seat.keyboard_node.set(seat.state.root.clone());
            // log::info!("keyboard_node = root");
            if focus_last {
                seat.output
                    .get()
                    .node_do_focus(&seat, Direction::Unspecified);
            }
        }
    }

    pub fn for_each_kb_focus<F: FnMut(Rc<WlSeatGlobal>)>(&self, mut f: F) {
        self.kb_foci.iter().for_each(|(_, s)| f(s));
    }

    pub fn for_each_pointer_focus<F: FnMut(Rc<WlSeatGlobal>)>(&self, mut f: F) {
        self.pointer_foci.iter().for_each(|(_, s)| f(s));
    }

    pub fn destroy_node(&self, node: &dyn Node) {
        self.destroy_node2(node, true);
    }

    fn destroy_node2(&self, node: &dyn Node, focus_last: bool) {
        // NOTE: Also called by set_visible(false)

        while let Some((_, seat)) = self.pointer_grabs.pop() {
            seat.pointer_owner.revert_to_default(&seat);
        }
        let node_id = node.node_id();
        while let Some((_, seat)) = self.dnd_targets.pop() {
            seat.pointer_owner.dnd_target_removed(&seat);
        }
        while let Some((_, seat)) = self.pointer_foci.pop() {
            let mut ps = seat.pointer_stack.borrow_mut();
            while let Some(last) = ps.pop() {
                if last.node_id() == node_id {
                    break;
                }
                last.node_seat_state().leave(&seat);
                last.node_on_leave(&seat);
            }
            seat.pointer_stack_modified.set(true);
            seat.state.tree_changed();
        }
        self.release_kb_focus2(focus_last);
    }

    pub fn set_visible(&self, node: &dyn Node, visible: bool) {
        if !visible {
            if !self.kb_foci.is_empty() {
                node.node_active_changed(false);
            }
            self.destroy_node2(node, false);
        }
    }

    pub fn on_seat_remove(&self, seat: &WlSeatGlobal) {
        self.kb_foci.remove(&seat.id);
        self.pointer_foci.remove(&seat.id);
        self.dnd_targets.remove(&seat.id);
        self.pointer_grabs.remove(&seat.id);
    }

    pub fn clear(&self) {
        self.kb_foci.clear();
        self.pointer_foci.clear();
        self.dnd_targets.clear();
        self.pointer_grabs.clear();
    }
}

impl WlSeatGlobal {
    pub fn event(self: &Rc<Self>, event: InputEvent) {
        match event {
            InputEvent::Key {
                time_usec,
                key,
                state,
            } => self.key_event(time_usec, key, state),
            InputEvent::ConnectorPosition {
                time_usec,
                connector,
                x,
                y,
            } => self.connector_position_event(time_usec, connector, x, y),
            InputEvent::Motion {
                dx,
                dy,
                dx_unaccelerated,
                dy_unaccelerated,
                time_usec,
            } => self.motion_event(time_usec, dx, dy, dx_unaccelerated, dy_unaccelerated),
            InputEvent::Button {
                time_usec,
                button,
                state,
            } => self.pointer_owner.button(self, time_usec, button, state),

            InputEvent::AxisSource { source } => self.pointer_owner.axis_source(source),
            InputEvent::AxisDiscrete { dist, axis } => self.pointer_owner.axis_discrete(dist, axis),
            InputEvent::Axis { dist, axis } => self.pointer_owner.axis(dist, axis),
            InputEvent::AxisStop { axis } => self.pointer_owner.axis_stop(axis),
            InputEvent::AxisFrame { time_usec } => self.pointer_owner.frame(self, time_usec),
        }
    }

    fn connector_position_event(
        self: &Rc<Self>,
        time_usec: u64,
        connector: ConnectorId,
        mut x: Fixed,
        mut y: Fixed,
    ) {
        let output = match self.state.outputs.get(&connector) {
            Some(o) => o,
            _ => return,
        };
        self.output.set(output.node.clone());
        let pos = output.node.global.pos.get();
        x += Fixed::from_int(pos.x1());
        y += Fixed::from_int(pos.y1());
        self.set_new_position(time_usec, x, y);
    }

    fn motion_event(
        self: &Rc<Self>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        self.pointer_owner.relative_motion(
            self,
            time_usec,
            dx,
            dy,
            dx_unaccelerated,
            dy_unaccelerated,
        );
        let (mut x, mut y) = self.pos.get();
        x += dx;
        y += dy;
        let output = self.output.get();
        let pos = output.global.pos.get();
        let mut x_int = x.round_down();
        let mut y_int = y.round_down();
        if !pos.contains(x_int, y_int) {
            'warp: {
                let outputs = self.state.outputs.lock();
                for output in outputs.values() {
                    if output.node.global.pos.get().contains(x_int, y_int) {
                        self.output.set(output.node.clone());
                        break 'warp;
                    }
                }
                if x_int < pos.x1() {
                    x_int = pos.x1();
                } else if x_int >= pos.x2() {
                    x_int = pos.x2() - 1;
                }
                if y_int < pos.y1() {
                    y_int = pos.y1();
                } else if y_int >= pos.y2() {
                    y_int = pos.y2() - 1;
                }
                x = x.apply_fract(x_int);
                y = y.apply_fract(y_int);
            }
        }
        self.set_new_position(time_usec, x, y);
    }

    fn key_event(&self, time_usec: u64, key: u32, state: KeyState) {
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
            if !self.state.lock.locked.get() && state == wl_keyboard::PRESSED {
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
            node.node_on_key(self, time_usec, key, state);
        } else if let Some(config) = self.state.config.get() {
            for shortcut in shortcuts {
                config.invoke_shortcut(self.id(), &shortcut);
            }
        }
        if let Some(mods) = new_mods {
            node.node_on_mods(self, mods);
        }
    }
}

impl WlSeatGlobal {
    pub(super) fn pointer_node(&self) -> Option<Rc<dyn Node>> {
        self.pointer_stack.borrow().last().cloned()
    }

    pub fn move_(&self, node: &Rc<FloatNode>) {
        self.move_.set(true);
        self.move_start_pos.set(self.pos.get());
        let ex = node.position.get();
        self.extents_start_pos.set((ex.x1(), ex.y1()));
    }

    pub fn focus_toplevel(self: &Rc<Self>, n: Rc<dyn ToplevelNode>) {
        let node = match n.tl_focus_child(self.id) {
            Some(n) => n,
            _ => n.tl_into_node(),
        };
        self.focus_node(node);
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

    fn offer_selection<T: ipc::IpcVtable>(
        &self,
        field: &CloneCell<Option<Rc<T::Source>>>,
        client: &Rc<Client>,
    ) {
        match field.get() {
            Some(sel) => ipc::offer_source_to::<T>(&sel, client),
            None => T::for_each_device(self, client.id, |dd| {
                T::send_selection(dd, None);
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

    fn for_each_relative_pointer<C>(&self, client: ClientId, mut f: C)
    where
        C: FnMut(&Rc<ZwpRelativePointerV1>),
    {
        self.for_each_seat(0, client, |seat| {
            let pointers = seat.relative_pointers.lock();
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
                if dd.version >= ver {
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
                if dd.version >= ver {
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
        // client.flush();
    }

    fn surface_relative_pointer_event<F>(&self, surface: &WlSurface, mut f: F)
    where
        F: FnMut(&Rc<ZwpRelativePointerV1>),
    {
        let client = &surface.client;
        self.for_each_relative_pointer(client.id, |p| {
            f(p);
        });
    }

    fn surface_kb_event<F>(&self, ver: u32, surface: &WlSurface, mut f: F)
    where
        F: FnMut(&Rc<WlKeyboard>),
    {
        let client = &surface.client;
        self.for_each_kb(ver, client.id, |p| {
            f(p);
        });
        // client.flush();
    }

    fn set_new_position(self: &Rc<Self>, time_usec: u64, x: Fixed, y: Fixed) {
        self.pos_time_usec.set(time_usec);
        self.pos.set((x, y));
        if let Some(cursor) = self.cursor.get() {
            cursor.set_position(x.round_down(), y.round_down());
        }
        self.changes.or_assign(CHANGE_CURSOR_MOVED);
        self.apply_changes();
    }

    pub fn clear_shortcuts(&self) {
        self.shortcuts.clear();
    }

    pub fn add_shortcut(&self, mods: Modifiers, keysym: KeySym) {
        self.shortcuts.set((mods.0, keysym.0), mods);
    }

    pub fn remove_shortcut(&self, mods: Modifiers, keysym: KeySym) {
        self.shortcuts.remove(&(mods.0, keysym.0));
    }

    pub fn trigger_tree_changed(&self) {
        // log::info!("trigger_tree_changed");
        self.tree_changed.trigger();
    }

    pub(super) fn apply_changes(self: &Rc<Self>) {
        self.state.damage();
        self.pointer_owner.apply_changes(self);
        self.changes.set(0);
    }
}

// Button callbacks
impl WlSeatGlobal {
    pub fn button_surface(
        self: &Rc<Self>,
        surface: &Rc<WlSurface>,
        time_usec: u64,
        button: u32,
        state: KeyState,
        serial: u32,
    ) {
        let (state, pressed) = match state {
            KeyState::Released => (wl_pointer::RELEASED, false),
            KeyState::Pressed => (wl_pointer::PRESSED, true),
        };
        let time = (time_usec / 1000) as u32;
        self.surface_pointer_event(0, surface, |p| p.send_button(serial, time, button, state));
        self.surface_pointer_frame(surface);
        if pressed {
            if let Some(node) = surface.get_focus_node(self.id) {
                self.focus_node(node);
            }
        }
    }
}

// Scroll callbacks
impl WlSeatGlobal {
    pub fn scroll_surface(&self, surface: &WlSurface, event: &PendingScroll) {
        if let Some(source) = event.source.get() {
            let since = if source >= WHEEL_TILT {
                WHEEL_TILT_SINCE_VERSION
            } else {
                AXIS_SOURCE_SINCE_VERSION
            };
            self.surface_pointer_event(since, surface, |p| p.send_axis_source(source));
        }
        let time = (event.time_usec.get() / 1000) as _;
        for i in 0..1 {
            if let Some(delta) = event.discrete[i].get() {
                self.surface_pointer_event(AXIS_DISCRETE_SINCE_VERSION, surface, |p| {
                    p.send_axis_discrete(i as _, delta)
                });
            }
            if let Some(delta) = event.axis[i].get() {
                self.surface_pointer_event(0, surface, |p| p.send_axis(time, i as _, delta));
            }
            if event.stop[i].get() {
                self.surface_pointer_event(AXIS_STOP_SINCE_VERSION, surface, |p| {
                    p.send_axis_stop(time, i as _)
                });
            }
        }
        self.surface_pointer_frame(surface);
    }
}

// Motion callbacks
impl WlSeatGlobal {
    pub fn motion_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        let time = (self.pos_time_usec.get() / 1000) as u32;
        self.surface_pointer_event(0, n, |p| p.send_motion(time, x, y));
        self.surface_pointer_frame(n);
    }
}

// Relative motion callbacks
impl WlSeatGlobal {
    pub fn relative_motion_surface(
        &self,
        surface: &WlSurface,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        self.surface_relative_pointer_event(surface, |p| {
            p.send_relative_motion(time_usec, dx, dy, dx_unaccelerated, dy_unaccelerated);
        });
    }
}

// Enter callbacks
impl WlSeatGlobal {
    pub fn enter_toplevel(self: &Rc<Self>, n: Rc<dyn ToplevelNode>) {
        if n.tl_accepts_keyboard_focus() && self.changes.get().contains(CHANGE_CURSOR_MOVED) {
            self.focus_toplevel(n);
        }
    }

    pub fn enter_popup(self: &Rc<Self>, _n: &Rc<XdgPopup>) {
        // self.focus_xdg_surface(&n.xdg);
    }

    pub fn enter_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        let serial = n.client.next_serial();
        n.client.last_enter_serial.set(serial);
        self.surface_pointer_event(0, n, |p| p.send_enter(serial, n.id, x, y));
        self.surface_pointer_frame(n);
    }
}

// Leave callbacks
impl WlSeatGlobal {
    pub fn leave_surface(&self, n: &WlSurface) {
        let serial = n.client.next_serial();
        self.surface_pointer_event(0, n, |p| p.send_leave(serial, n.id));
        self.surface_pointer_frame(n);
    }
}

// Unfocus callbacks
impl WlSeatGlobal {
    pub fn unfocus_surface(&self, surface: &WlSurface) {
        let serial = surface.client.next_serial();
        self.surface_kb_event(0, surface, |k| k.send_leave(serial, surface.id))
    }
}

// Focus callbacks
impl WlSeatGlobal {
    pub fn focus_surface(&self, surface: &WlSurface) {
        let pressed_keys: Vec<_> = self.pressed_keys.borrow().iter().copied().collect();
        let serial = surface.client.next_serial();
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
        let serial = surface.client.next_serial();
        self.surface_kb_event(0, surface, |k| {
            k.send_modifiers(serial, mods_depressed, mods_latched, mods_locked, group)
        });

        if self.keyboard_node.get().node_client_id() != Some(surface.client.id) {
            self.offer_selection::<ClipboardIpc>(&self.selection, &surface.client);
            self.offer_selection::<PrimarySelectionIpc>(&self.primary_selection, &surface.client);
        }
    }
}

// Key callbacks
impl WlSeatGlobal {
    pub fn key_surface(&self, surface: &WlSurface, time_usec: u64, key: u32, state: u32) {
        let serial = surface.client.next_serial();
        let time = (time_usec / 1000) as _;
        self.surface_kb_event(0, surface, |k| k.send_key(serial, time, key, state));
    }
}

// Modifiers callbacks
impl WlSeatGlobal {
    pub fn mods_surface(&self, surface: &WlSurface, mods: ModifierState) {
        let serial = surface.client.next_serial();
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
        // surface.client.flush();
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
        // surface.client.flush();
    }

    pub fn dnd_surface_enter(
        &self,
        surface: &WlSurface,
        dnd: &Dnd,
        x: Fixed,
        y: Fixed,
        serial: u32,
    ) {
        if let Some(src) = &dnd.src {
            ipc::offer_source_to::<ClipboardIpc>(src, &surface.client);
            src.for_each_data_offer(|offer| {
                offer.device.send_enter(surface.id, x, y, offer.id, serial);
                offer.send_source_actions();
            })
        } else if surface.client.id == dnd.client.id {
            self.for_each_data_device(0, dnd.client.id, |dd| {
                dd.send_enter(surface.id, x, y, WlDataOfferId::NONE, serial);
            })
        }
        // surface.client.flush();
    }

    pub fn dnd_surface_motion(
        &self,
        surface: &WlSurface,
        dnd: &Dnd,
        time_usec: u64,
        x: Fixed,
        y: Fixed,
    ) {
        if dnd.src.is_some() || surface.client.id == dnd.client.id {
            self.for_each_data_device(0, surface.client.id, |dd| {
                dd.send_motion(time_usec, x, y);
            })
        }
        // surface.client.flush();
    }
}
