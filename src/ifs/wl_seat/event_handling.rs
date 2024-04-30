use {
    crate::{
        backend::{ConnectorId, InputDeviceId, InputEvent, KeyState, AXIS_120},
        client::ClientId,
        config::InvokedShortcut,
        fixed::Fixed,
        ifs::{
            ipc::{
                wl_data_device::{ClipboardIpc, WlDataDevice},
                x_data_device::{XClipboardIpc, XPrimarySelectionIpc},
                zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
                zwp_primary_selection_device_v1::{
                    PrimarySelectionIpc, ZwpPrimarySelectionDeviceV1,
                },
                DynDataSource,
            },
            wl_seat::{
                tablet::{TabletPad, TabletPadId, TabletTool, TabletToolId},
                text_input::TextDisconnectReason,
                wl_keyboard::{self, WlKeyboard},
                wl_pointer::{
                    self, PendingScroll, WlPointer, AXIS_DISCRETE_SINCE_VERSION,
                    AXIS_RELATIVE_DIRECTION_SINCE_VERSION, AXIS_SOURCE_SINCE_VERSION,
                    AXIS_STOP_SINCE_VERSION, AXIS_VALUE120_SINCE_VERSION, IDENTICAL, INVERTED,
                    POINTER_FRAME_SINCE_VERSION, WHEEL_TILT, WHEEL_TILT_SINCE_VERSION,
                },
                zwp_pointer_constraints_v1::{ConstraintType, SeatConstraintStatus},
                zwp_relative_pointer_v1::ZwpRelativePointerV1,
                Dnd, SeatId, WlSeat, WlSeatGlobal, CHANGE_CURSOR_MOVED, CHANGE_TREE,
            },
            wl_surface::{xdg_surface::xdg_popup::XdgPopup, WlSurface},
        },
        object::Version,
        state::DeviceHandlerData,
        tree::{Direction, Node, ToplevelNode},
        utils::{bitflags::BitflagsExt, smallmap::SmallMap},
        wire::WlDataOfferId,
        xkbcommon::{KeyboardState, XkbState, XKB_KEY_DOWN, XKB_KEY_UP},
    },
    isnt::std_1::primitive::{IsntSlice2Ext, IsntSliceExt},
    jay_config::{
        input::SwitchEvent,
        keyboard::{
            mods::{Modifiers, CAPS, NUM, RELEASE},
            syms::{KeySym, SYM_Escape},
        },
    },
    smallvec::SmallVec,
    std::{cell::RefCell, collections::hash_map::Entry, rc::Rc},
};

#[derive(Default)]
pub struct NodeSeatState {
    pointer_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    kb_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    gesture_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    pointer_grabs: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    dnd_targets: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    tablet_pad_foci: SmallMap<TabletPadId, Rc<TabletPad>, 1>,
    tablet_tool_foci: SmallMap<TabletToolId, Rc<TabletTool>, 1>,
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

    pub(super) fn gesture_begin(&self, seat: &Rc<WlSeatGlobal>) {
        self.gesture_foci.insert(seat.id, seat.clone());
    }

    pub(super) fn gesture_end(&self, seat: &WlSeatGlobal) {
        self.gesture_foci.remove(&seat.id);
    }

    pub(super) fn add_pointer_grab(&self, seat: &Rc<WlSeatGlobal>) {
        self.pointer_grabs.insert(seat.id, seat.clone());
    }

    pub(super) fn remove_pointer_grab(&self, seat: &WlSeatGlobal) {
        self.pointer_grabs.remove(&seat.id);
    }

    pub(super) fn add_tablet_pad_focus(&self, pad: &Rc<TabletPad>) {
        self.tablet_pad_foci.insert(pad.id, pad.clone());
    }

    pub(super) fn remove_tablet_pad_focus(&self, pad: &TabletPad) {
        self.tablet_pad_foci.remove(&pad.id);
    }

    pub(super) fn add_tablet_tool_focus(&self, tool: &Rc<TabletTool>) {
        self.tablet_tool_foci.insert(tool.id, tool.clone());
    }

    pub(super) fn remove_tablet_tool_focus(&self, tool: &TabletTool) {
        self.tablet_tool_foci.remove(&tool.id);
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
            seat.kb_owner.set_kb_node(&seat, seat.state.root.clone());
            // log::info!("keyboard_node = root");
            if focus_last {
                seat.get_output()
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

        while let Some((_, seat)) = self.gesture_foci.pop() {
            seat.gesture_owner.revert_to_default(&seat);
        }
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
        while let Some((_, tool)) = self.tablet_tool_foci.pop() {
            tool.tool_owner.focus_root(&tool);
        }
        while let Some((_, pad)) = self.tablet_pad_foci.pop() {
            pad.pad_owner.focus_root(&pad);
        }
        self.release_kb_focus2(focus_last);
    }

    pub fn set_visible(&self, node: &dyn Node, visible: bool) {
        if !visible {
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
    pub fn event(self: &Rc<Self>, dev: &DeviceHandlerData, event: InputEvent) {
        match event {
            InputEvent::Key { time_usec, .. }
            | InputEvent::ConnectorPosition { time_usec, .. }
            | InputEvent::Motion { time_usec, .. }
            | InputEvent::Button { time_usec, .. }
            | InputEvent::AxisFrame { time_usec, .. }
            | InputEvent::SwipeBegin { time_usec, .. }
            | InputEvent::SwipeUpdate { time_usec, .. }
            | InputEvent::SwipeEnd { time_usec, .. }
            | InputEvent::PinchBegin { time_usec, .. }
            | InputEvent::PinchUpdate { time_usec, .. }
            | InputEvent::PinchEnd { time_usec, .. }
            | InputEvent::HoldBegin { time_usec, .. }
            | InputEvent::HoldEnd { time_usec, .. }
            | InputEvent::SwitchEvent { time_usec, .. }
            | InputEvent::TabletToolChanged { time_usec, .. }
            | InputEvent::TabletToolButton { time_usec, .. }
            | InputEvent::TabletPadButton { time_usec, .. }
            | InputEvent::TabletPadModeSwitch { time_usec, .. }
            | InputEvent::TabletPadRing { time_usec, .. }
            | InputEvent::TabletPadStrip { time_usec, .. } => {
                self.last_input_usec.set(time_usec);
                if self.idle_notifications.is_not_empty() {
                    for (_, notification) in self.idle_notifications.lock().drain() {
                        notification.resume.trigger();
                    }
                }
            }
            InputEvent::AxisPx { .. }
            | InputEvent::AxisSource { .. }
            | InputEvent::AxisStop { .. }
            | InputEvent::Axis120 { .. }
            | InputEvent::TabletToolAdded { .. }
            | InputEvent::TabletToolRemoved { .. } => {}
        }
        match event {
            InputEvent::ConnectorPosition { .. }
            | InputEvent::Motion { .. }
            | InputEvent::Button { .. }
            | InputEvent::AxisFrame { .. }
            | InputEvent::SwipeBegin { .. }
            | InputEvent::SwipeUpdate { .. }
            | InputEvent::SwipeEnd { .. }
            | InputEvent::PinchBegin { .. }
            | InputEvent::PinchUpdate { .. }
            | InputEvent::PinchEnd { .. }
            | InputEvent::HoldBegin { .. }
            | InputEvent::HoldEnd { .. } => {
                self.pointer_cursor.activate();
            }
            InputEvent::Key { .. } => {}
            InputEvent::AxisPx { .. } => {}
            InputEvent::AxisSource { .. } => {}
            InputEvent::AxisStop { .. } => {}
            InputEvent::Axis120 { .. } => {}
            InputEvent::SwitchEvent { .. } => {}
            InputEvent::TabletToolAdded { .. } => {}
            InputEvent::TabletToolChanged { .. } => {}
            InputEvent::TabletToolButton { .. } => {}
            InputEvent::TabletToolRemoved { .. } => {}
            InputEvent::TabletPadButton { .. } => {}
            InputEvent::TabletPadModeSwitch { .. } => {}
            InputEvent::TabletPadRing { .. } => {}
            InputEvent::TabletPadStrip { .. } => {}
        }
        match event {
            InputEvent::Key {
                time_usec,
                key,
                state,
            } => self.key_event(time_usec, key, state, || dev.get_effective_xkb_state(self)),
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
            } => self.button_event(time_usec, button, state),

            InputEvent::AxisSource { source } => self.pointer_owner.axis_source(source),
            InputEvent::Axis120 {
                dist,
                axis,
                inverted,
            } => self.pointer_owner.axis_120(dist, axis, inverted),
            InputEvent::AxisPx {
                dist,
                axis,
                inverted,
            } => self.pointer_owner.axis_px(dist, axis, inverted),
            InputEvent::AxisStop { axis } => self.pointer_owner.axis_stop(axis),
            InputEvent::AxisFrame { time_usec } => self.pointer_owner.frame(dev, self, time_usec),
            InputEvent::SwipeBegin {
                time_usec,
                finger_count,
            } => self.swipe_begin(time_usec, finger_count),
            InputEvent::SwipeUpdate {
                time_usec,
                dx,
                dy,
                dx_unaccelerated,
                dy_unaccelerated,
            } => self.swipe_update(time_usec, dx, dy, dx_unaccelerated, dy_unaccelerated),
            InputEvent::SwipeEnd {
                time_usec,
                cancelled,
            } => self.swipe_end(time_usec, cancelled),
            InputEvent::PinchBegin {
                time_usec,
                finger_count,
            } => self.pinch_begin(time_usec, finger_count),
            InputEvent::PinchUpdate {
                time_usec,
                dx,
                dy,
                dx_unaccelerated,
                dy_unaccelerated,
                scale,
                rotation,
            } => self.pinch_update(
                time_usec,
                dx,
                dy,
                dx_unaccelerated,
                dy_unaccelerated,
                scale,
                rotation,
            ),
            InputEvent::PinchEnd {
                time_usec,
                cancelled,
            } => self.pinch_end(time_usec, cancelled),
            InputEvent::HoldBegin {
                time_usec,
                finger_count,
            } => self.hold_begin(time_usec, finger_count),
            InputEvent::HoldEnd {
                time_usec,
                cancelled,
            } => self.hold_end(time_usec, cancelled),
            InputEvent::SwitchEvent { time_usec, event } => {
                self.switch_event(dev.device.id(), time_usec, event)
            }
            InputEvent::TabletToolAdded { time_usec, init } => {
                self.tablet_handle_new_tool(time_usec, &init)
            }
            InputEvent::TabletToolChanged {
                time_usec,
                id,
                changes: change,
            } => self.tablet_event_tool_changes(id, time_usec, dev.get_rect(&self.state), &change),
            InputEvent::TabletToolButton {
                time_usec,
                id,
                button,
                state,
            } => self.tablet_event_tool_button(id, time_usec, button, state),
            InputEvent::TabletToolRemoved { time_usec, id } => {
                self.tablet_handle_remove_tool(time_usec, id)
            }
            InputEvent::TabletPadButton {
                time_usec,
                id,
                button,
                state,
            } => self.tablet_event_pad_button(id, time_usec, button, state),
            InputEvent::TabletPadModeSwitch {
                time_usec,
                pad,
                group,
                mode,
            } => self.tablet_event_pad_mode_switch(pad, time_usec, group, mode),
            InputEvent::TabletPadRing {
                time_usec,
                pad,
                ring,
                source,
                angle,
            } => self.tablet_event_pad_ring(pad, ring, source, angle, time_usec),
            InputEvent::TabletPadStrip {
                time_usec,
                pad,
                strip,
                source,
                position,
            } => self.tablet_event_pad_strip(pad, strip, source, position, time_usec),
        }
    }

    fn connector_position_event(
        self: &Rc<Self>,
        time_usec: u64,
        connector: ConnectorId,
        mut x: Fixed,
        mut y: Fixed,
    ) {
        let output = match self.state.root.outputs.get(&connector) {
            Some(o) => o,
            _ => return,
        };
        let pos = output.global.pos.get();
        x += Fixed::from_int(pos.x1());
        y += Fixed::from_int(pos.y1());
        (x, y) = self.pointer_cursor.set_position(x, y);
        if let Some(c) = self.constraint.get() {
            if c.ty == ConstraintType::Lock || !c.contains(x.round_down(), y.round_down()) {
                c.deactivate();
            }
        }
        self.state.for_each_seat_tester(|t| {
            t.send_pointer_abs(self.id, time_usec, x, y);
        });
        self.cursor_moved(time_usec);
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
        let constraint = self.constraint.get();
        let locked = match &constraint {
            Some(c) if c.ty == ConstraintType::Lock => true,
            _ => false,
        };
        let (mut x, mut y) = self.pointer_cursor.position();
        if !locked {
            x += dx;
            y += dy;
            if let Some(c) = &constraint {
                let surface_pos = c.surface.buffer_abs_pos.get();
                let (x_rel, y_rel) = (x - surface_pos.x1(), y - surface_pos.y1());
                let contained = surface_pos.contains(x.round_down(), y.round_down())
                    && c.contains(x_rel.round_down(), y_rel.round_down());
                if !contained {
                    let (x_rel, y_rel) = c.warp(x_rel, y_rel);
                    (x, y) = (x_rel + surface_pos.x1(), y_rel + surface_pos.y1());
                }
            }
        }
        self.state.for_each_seat_tester(|t| {
            t.send_pointer_rel(
                self.id,
                time_usec,
                x,
                y,
                dx,
                dy,
                dx_unaccelerated,
                dy_unaccelerated,
            );
        });
        self.pointer_cursor.set_position(x, y);
        self.cursor_moved(time_usec);
    }

    fn button_event(self: &Rc<Self>, time_usec: u64, button: u32, state: KeyState) {
        self.state.for_each_seat_tester(|t| {
            t.send_button(self.id, time_usec, button, state);
        });
        self.pointer_owner.button(self, time_usec, button, state);
    }

    fn swipe_begin(self: &Rc<Self>, time_usec: u64, finger_count: u32) {
        self.state.for_each_seat_tester(|t| {
            t.send_swipe_begin(self.id, time_usec, finger_count);
        });
        self.gesture_owner
            .swipe_begin(self, time_usec, finger_count)
    }

    fn swipe_update(
        self: &Rc<Self>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        self.state.for_each_seat_tester(|t| {
            t.send_swipe_update(
                self.id,
                time_usec,
                dx,
                dy,
                dx_unaccelerated,
                dy_unaccelerated,
            );
        });
        self.gesture_owner.swipe_update(self, time_usec, dx, dy)
    }

    fn swipe_end(self: &Rc<Self>, time_usec: u64, cancelled: bool) {
        self.state.for_each_seat_tester(|t| {
            t.send_swipe_end(self.id, time_usec, cancelled);
        });
        self.gesture_owner.swipe_end(self, time_usec, cancelled)
    }

    fn pinch_begin(self: &Rc<Self>, time_usec: u64, finger_count: u32) {
        self.state.for_each_seat_tester(|t| {
            t.send_pinch_begin(self.id, time_usec, finger_count);
        });
        self.gesture_owner
            .pinch_begin(self, time_usec, finger_count)
    }

    fn pinch_update(
        self: &Rc<Self>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        self.state.for_each_seat_tester(|t| {
            t.send_pinch_update(
                self.id,
                time_usec,
                dx,
                dy,
                dx_unaccelerated,
                dy_unaccelerated,
                scale,
                rotation,
            );
        });
        self.gesture_owner
            .pinch_update(self, time_usec, dx, dy, scale, rotation)
    }

    fn pinch_end(self: &Rc<Self>, time_usec: u64, cancelled: bool) {
        self.state.for_each_seat_tester(|t| {
            t.send_pinch_end(self.id, time_usec, cancelled);
        });
        self.gesture_owner.pinch_end(self, time_usec, cancelled)
    }

    fn hold_begin(self: &Rc<Self>, time_usec: u64, finger_count: u32) {
        self.state.for_each_seat_tester(|t| {
            t.send_hold_begin(self.id, time_usec, finger_count);
        });
        self.gesture_owner.hold_begin(self, time_usec, finger_count)
    }

    fn hold_end(self: &Rc<Self>, time_usec: u64, cancelled: bool) {
        self.state.for_each_seat_tester(|t| {
            t.send_hold_end(self.id, time_usec, cancelled);
        });
        self.gesture_owner.hold_end(self, time_usec, cancelled)
    }

    fn switch_event(self: &Rc<Self>, dev: InputDeviceId, time_usec: u64, event: SwitchEvent) {
        self.state.for_each_seat_tester(|t| {
            t.send_switch_event(self.id, dev, time_usec, event);
        });
        if let Some(config) = self.state.config.get() {
            config.switch_event(self.id, dev, event);
        }
    }

    pub(super) fn key_event<F>(
        self: &Rc<Self>,
        time_usec: u64,
        key: u32,
        key_state: KeyState,
        mut get_state: F,
    ) where
        F: FnMut() -> Rc<RefCell<XkbState>>,
    {
        let mut xkb_state_rc = get_state();
        let mut xkb_state = xkb_state_rc.borrow_mut();
        let (state, xkb_dir) = {
            match key_state {
                KeyState::Released => {
                    if xkb_state.kb_state.pressed_keys.not_contains(&key) {
                        return;
                    }
                    (wl_keyboard::RELEASED, XKB_KEY_UP)
                }
                KeyState::Pressed => {
                    if xkb_state.kb_state.pressed_keys.contains(&key) {
                        return;
                    }
                    (wl_keyboard::PRESSED, XKB_KEY_DOWN)
                }
            }
        };
        let mut shortcuts = SmallVec::<[_; 1]>::new();
        let new_mods;
        {
            let mut mods = xkb_state.mods().mods_effective & !(CAPS.0 | NUM.0);
            if state == wl_keyboard::RELEASED {
                mods |= RELEASE.0;
            }
            let scs = &*self.shortcuts.borrow();
            let keysyms = xkb_state.unmodified_keysyms(key);
            for &sym in keysyms {
                if sym == SYM_Escape.0 && mods == 0 {
                    self.pointer_owner.revert_to_default(self);
                }
                if !self.state.lock.locked.get() {
                    if let Some(key_mods) = scs.get(&sym) {
                        for (key_mods, mask) in key_mods {
                            if mods & mask == key_mods {
                                shortcuts.push(InvokedShortcut {
                                    unmasked_mods: Modifiers(mods),
                                    effective_mods: Modifiers(key_mods),
                                    sym: KeySym(sym),
                                });
                            }
                        }
                    }
                }
            }
            new_mods = xkb_state.update(key, xkb_dir);
        }
        self.state.for_each_seat_tester(|t| {
            t.send_key(self.id, time_usec, key, key_state);
        });
        let node = self.keyboard_node.get();
        let input_method_grab = self.input_method_grab.get();
        let mut forward = true;
        if shortcuts.is_not_empty() {
            self.forward.set(state == wl_keyboard::RELEASED);
            if let Some(config) = self.state.config.get() {
                let id = xkb_state.kb_state.id;
                drop(xkb_state);
                for shortcut in shortcuts {
                    config.invoke_shortcut(self.id(), &shortcut);
                }
                xkb_state_rc = get_state();
                xkb_state = xkb_state_rc.borrow_mut();
                if id != xkb_state.kb_state.id {
                    return;
                }
            }
            forward = self.forward.get();
        }
        if forward {
            match &input_method_grab {
                Some(g) => g.on_key(time_usec, key, state, &xkb_state.kb_state),
                _ => node.node_on_key(self, time_usec, key, state, &xkb_state.kb_state),
            }
        }
        if new_mods {
            self.state.for_each_seat_tester(|t| {
                t.send_modifiers(self.id, &xkb_state.kb_state.mods);
            });
            match &input_method_grab {
                Some(g) => g.on_modifiers(&xkb_state.kb_state),
                _ => node.node_on_mods(self, &xkb_state.kb_state),
            }
        }
        match key_state {
            KeyState::Released => {
                xkb_state.kb_state.pressed_keys.remove(&key);
            }
            KeyState::Pressed => {
                xkb_state.kb_state.pressed_keys.insert(key);
            }
        }
        drop(xkb_state);
        self.latest_kb_state.set(xkb_state_rc);
    }
}

impl WlSeatGlobal {
    pub fn pointer_node(&self) -> Option<Rc<dyn Node>> {
        self.pointer_stack.borrow().last().cloned()
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

    fn for_each_seat<C>(&self, ver: Version, client: ClientId, mut f: C)
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

    fn for_each_pointer<C>(&self, ver: Version, client: ClientId, mut f: C)
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
        self.for_each_seat(Version::ALL, client, |seat| {
            let pointers = seat.relative_pointers.lock();
            for pointer in pointers.values() {
                f(pointer);
            }
        })
    }

    fn for_each_kb<C>(&self, ver: Version, client: ClientId, mut f: C)
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

    pub fn for_each_data_device<C>(&self, ver: Version, client: ClientId, mut f: C)
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

    pub fn for_each_primary_selection_device<C>(&self, ver: Version, client: ClientId, mut f: C)
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

    pub fn for_each_wlr_data_device<C>(&self, ver: Version, mut f: C)
    where
        C: FnMut(&Rc<ZwlrDataControlDeviceV1>),
    {
        for dd in self.wlr_data_devices.lock().values() {
            if dd.version >= ver {
                f(dd);
            }
        }
    }

    fn surface_pointer_frame(&self, surface: &WlSurface) {
        self.surface_pointer_event(POINTER_FRAME_SINCE_VERSION, surface, |p| p.send_frame());
    }

    fn surface_pointer_event<F>(&self, ver: Version, surface: &WlSurface, mut f: F)
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

    pub fn surface_kb_event<F>(&self, ver: Version, surface: &WlSurface, mut f: F)
    where
        F: FnMut(&Rc<WlKeyboard>),
    {
        let client = &surface.client;
        self.for_each_kb(ver, client.id, |p| {
            f(p);
        });
        // client.flush();
    }

    fn cursor_moved(self: &Rc<Self>, time_usec: u64) {
        self.pos_time_usec.set(time_usec);
        self.changes.or_assign(CHANGE_CURSOR_MOVED);
        self.apply_changes();
    }

    pub fn clear_shortcuts(&self) {
        self.shortcuts.borrow_mut().clear();
    }

    pub fn add_shortcut(&self, mod_mask: Modifiers, mods: Modifiers, keysym: KeySym) {
        self.shortcuts
            .borrow_mut()
            .entry(keysym.0)
            .or_default()
            .insert(mods.0, mod_mask.0);
    }

    pub fn remove_shortcut(&self, mods: Modifiers, keysym: KeySym) {
        if let Entry::Occupied(mut oe) = self.shortcuts.borrow_mut().entry(keysym.0) {
            oe.get_mut().remove(&mods.0);
            if oe.get().is_empty() {
                oe.remove();
            }
        }
    }

    pub fn trigger_tree_changed(&self) {
        // log::info!("trigger_tree_changed");
        self.tree_changed.trigger();
    }

    pub(super) fn apply_changes(self: &Rc<Self>) {
        self.state.damage();
        self.pointer_owner.apply_changes(self);
        if self.changes.get().contains(CHANGE_TREE) {
            self.tablet_apply_changes();
        }
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
        self.surface_pointer_event(Version::ALL, surface, |p| {
            p.send_button(serial, time, button, state)
        });
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
        self.for_each_pointer(Version::ALL, surface.client.id, |p| {
            for i in 0..1 {
                let axis = i as _;
                if let Some(delta) = event.v120[i].get() {
                    if p.seat.version >= AXIS_VALUE120_SINCE_VERSION {
                        p.send_axis_value120(axis, delta);
                    } else if p.seat.version >= AXIS_DISCRETE_SINCE_VERSION {
                        p.send_axis_discrete(axis, delta / AXIS_120);
                    }
                }
                if let Some(delta) = event.px[i].get() {
                    if p.seat.version >= AXIS_RELATIVE_DIRECTION_SINCE_VERSION {
                        let direction = match event.inverted[i].get() {
                            false => IDENTICAL,
                            true => INVERTED,
                        };
                        p.send_axis_relative_direction(axis, direction);
                    }
                    p.send_axis(time, axis, delta);
                }
                if p.seat.version >= AXIS_STOP_SINCE_VERSION && event.stop[i].get() {
                    p.send_axis_stop(time, axis);
                }
            }
            if p.seat.version >= POINTER_FRAME_SINCE_VERSION {
                p.send_frame();
            }
        });
    }
}

// Motion callbacks
impl WlSeatGlobal {
    pub fn motion_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        'send_motion: {
            if let Some(constraint) = self.constraint.get() {
                if constraint.ty == ConstraintType::Lock {
                    break 'send_motion;
                }
            }
            let time = (self.pos_time_usec.get() / 1000) as u32;
            self.surface_pointer_event(Version::ALL, n, |p| p.send_motion(time, x, y));
        }
        self.surface_pointer_frame(n);
        self.maybe_constrain(n, x, y);
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
        if n.tl_accepts_keyboard_focus()
            && self.changes.get().contains(CHANGE_CURSOR_MOVED)
            && self.focus_follows_mouse.get()
        {
            self.focus_toplevel(n);
        }
    }

    pub fn enter_popup(self: &Rc<Self>, _n: &Rc<XdgPopup>) {
        // self.focus_xdg_surface(&n.xdg);
    }

    pub fn enter_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        let serial = n.client.next_serial();
        n.client.last_enter_serial.set(serial);
        self.surface_pointer_event(Version::ALL, n, |p| p.send_enter(serial, n.id, x, y));
        self.surface_pointer_frame(n);
        for (_, constraint) in &n.constraints {
            if constraint.status.get() == SeatConstraintStatus::ActivatableOnFocus {
                constraint.status.set(SeatConstraintStatus::Inactive);
            }
        }
        self.maybe_constrain(n, x, y);
    }
}

// Leave callbacks
impl WlSeatGlobal {
    pub fn leave_surface(&self, n: &WlSurface) {
        let serial = n.client.next_serial();
        for (_, constraint) in &n.constraints {
            constraint.deactivate();
        }
        self.surface_pointer_event(Version::ALL, n, |p| p.send_leave(serial, n.id));
        self.surface_pointer_frame(n);
    }
}

// Unfocus callbacks
impl WlSeatGlobal {
    pub fn unfocus_surface(&self, surface: &WlSurface) {
        if let Some(ti) = self.text_input.take() {
            if let Some(con) = ti.connection.get() {
                con.disconnect(TextDisconnectReason::FocusLost);
            }
        }
        if let Some(tis) = self.text_inputs.borrow().get(&surface.client.id) {
            for ti in tis.lock().values() {
                ti.send_leave(surface);
                ti.send_done();
            }
        }

        let serial = surface.client.next_serial();
        self.surface_kb_event(Version::ALL, surface, |k| k.send_leave(serial, surface.id))
    }
}

// Focus callbacks
impl WlSeatGlobal {
    pub fn focus_surface(&self, surface: &WlSurface) {
        let kb_state = self.latest_kb_state.get();
        let kb_state = &*kb_state.borrow();
        let serial = surface.client.next_serial();
        self.surface_kb_event(Version::ALL, surface, |k| {
            k.enter(serial, surface.id, kb_state);
        });

        if self.keyboard_node.get().node_client_id() != Some(surface.client.id) {
            self.offer_selection_to_client::<ClipboardIpc, XClipboardIpc>(
                self.selection.get(),
                &surface.client,
            );
            self.offer_selection_to_client::<PrimarySelectionIpc, XPrimarySelectionIpc>(
                self.primary_selection.get(),
                &surface.client,
            );
        }

        if let Some(tis) = self.text_inputs.borrow_mut().get(&surface.client.id) {
            for ti in tis.lock().values() {
                ti.send_enter(surface);
                ti.send_done();
            }
        }
    }
}

// Key callbacks
impl WlSeatGlobal {
    pub fn key_surface(
        &self,
        surface: &WlSurface,
        time_usec: u64,
        key: u32,
        state: u32,
        kb_state: &KeyboardState,
    ) {
        let serial = surface.client.next_serial();
        let time = (time_usec / 1000) as _;
        self.surface_kb_event(Version::ALL, surface, |k| {
            k.on_key(serial, time, key, state, surface.id, kb_state);
        });
    }
}

// Modifiers callbacks
impl WlSeatGlobal {
    pub fn mods_surface(&self, surface: &WlSurface, kb_state: &KeyboardState) {
        let serial = surface.client.next_serial();
        self.surface_kb_event(Version::ALL, surface, |k| {
            k.on_mods_changed(serial, surface.id, kb_state)
        });
    }
}

// Dnd callbacks
impl WlSeatGlobal {
    pub fn dnd_surface_leave(&self, surface: &WlSurface, dnd: &Dnd) {
        if dnd.src.is_some() || surface.client.id == dnd.client.id {
            self.for_each_data_device(Version::ALL, surface.client.id, |dd| {
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
            self.for_each_data_device(Version::ALL, surface.client.id, |dd| {
                dd.send_drop();
            })
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
            if !surface.client.is_xwayland {
                src.clone().offer_to_regular_client(&surface.client);
            }
            src.for_each_data_offer(|offer| {
                offer.send_enter(surface.id, x, y, serial);
                offer.send_source_actions();
            })
        } else if surface.client.id == dnd.client.id {
            self.for_each_data_device(Version::ALL, dnd.client.id, |dd| {
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
            self.for_each_data_device(Version::ALL, surface.client.id, |dd| {
                dd.send_motion(time_usec, x, y);
            })
        }
        // surface.client.flush();
    }
}

// Gesture callbacks
impl WlSeatGlobal {
    pub fn swipe_begin_surface(&self, n: &WlSurface, time_usec: u64, finger_count: u32) {
        let serial = n.client.next_serial();
        self.swipe_bindings
            .for_each(n.client.id, Version::ALL, |obj| {
                obj.send_swipe_begin(n, serial, time_usec, finger_count)
            })
    }

    pub fn swipe_update_surface(&self, n: &WlSurface, time_usec: u64, dx: Fixed, dy: Fixed) {
        self.swipe_bindings
            .for_each(n.client.id, Version::ALL, |obj| {
                obj.send_swipe_update(time_usec, dx, dy)
            })
    }

    pub fn swipe_end_surface(&self, n: &WlSurface, time_usec: u64, cancelled: bool) {
        let serial = n.client.next_serial();
        self.swipe_bindings
            .for_each(n.client.id, Version::ALL, |obj| {
                obj.send_swipe_end(serial, time_usec, cancelled)
            })
    }

    pub fn pinch_begin_surface(&self, n: &WlSurface, time_usec: u64, finger_count: u32) {
        let serial = n.client.next_serial();
        self.pinch_bindings
            .for_each(n.client.id, Version::ALL, |obj| {
                obj.send_pinch_begin(n, serial, time_usec, finger_count)
            })
    }

    pub fn pinch_update_surface(
        &self,
        n: &WlSurface,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        self.pinch_bindings
            .for_each(n.client.id, Version::ALL, |obj| {
                obj.send_pinch_update(time_usec, dx, dy, scale, rotation)
            })
    }

    pub fn pinch_end_surface(&self, n: &WlSurface, time_usec: u64, cancelled: bool) {
        let serial = n.client.next_serial();
        self.pinch_bindings
            .for_each(n.client.id, Version::ALL, |obj| {
                obj.send_pinch_end(serial, time_usec, cancelled)
            })
    }

    pub fn hold_begin_surface(&self, n: &WlSurface, time_usec: u64, finger_count: u32) {
        let serial = n.client.next_serial();
        self.hold_bindings
            .for_each(n.client.id, Version::ALL, |obj| {
                obj.send_hold_begin(n, serial, time_usec, finger_count)
            })
    }

    pub fn hold_end_surface(&self, n: &WlSurface, time_usec: u64, cancelled: bool) {
        let serial = n.client.next_serial();
        self.hold_bindings
            .for_each(n.client.id, Version::ALL, |obj| {
                obj.send_hold_end(serial, time_usec, cancelled)
            })
    }
}
