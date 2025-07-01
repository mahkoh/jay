use {
    crate::{
        backend::{
            AXIS_120, AxisSource, ConnectorId, InputDeviceId, InputEvent, KeyState, ScrollAxis,
        },
        client::ClientId,
        config::InvokedShortcut,
        ei::ei_ifs::ei_seat::EiSeat,
        fixed::Fixed,
        ifs::{
            ipc::{
                offer_source_to_regular_client,
                wl_data_device::{ClipboardIpc, WlDataDevice},
                x_data_device::{XClipboardIpc, XPrimarySelectionIpc},
                zwp_primary_selection_device_v1::{
                    PrimarySelectionIpc, ZwpPrimarySelectionDeviceV1,
                },
            },
            wl_seat::{
                CHANGE_CURSOR_MOVED, CHANGE_TREE, Dnd, SeatId, WlSeat, WlSeatGlobal,
                tablet::{TabletPad, TabletPadId, TabletTool, TabletToolId},
                text_input::TextDisconnectReason,
                wl_keyboard::WlKeyboard,
                wl_pointer::{
                    self, AXIS_DISCRETE_SINCE_VERSION, AXIS_RELATIVE_DIRECTION_SINCE_VERSION,
                    AXIS_SOURCE_SINCE_VERSION, AXIS_STOP_SINCE_VERSION,
                    AXIS_VALUE120_SINCE_VERSION, IDENTICAL, INVERTED, POINTER_FRAME_SINCE_VERSION,
                    PendingScroll, WHEEL_TILT, WHEEL_TILT_SINCE_VERSION, WlPointer,
                },
                wl_touch::WlTouch,
                zwp_pointer_constraints_v1::{ConstraintType, SeatConstraintStatus},
                zwp_relative_pointer_v1::ZwpRelativePointerV1,
            },
            wl_surface::{WlSurface, xdg_surface::xdg_popup::XdgPopup},
        },
        kbvm::KbvmState,
        keyboard::KeyboardState,
        object::Version,
        rect::Rect,
        state::DeviceHandlerData,
        tree::{Direction, Node, ToplevelNode},
        utils::{
            bitflags::BitflagsExt, hash_map_ext::HashMapExt, smallmap::SmallMap,
            syncqueue::SyncQueue,
        },
        wire::WlDataOfferId,
    },
    isnt::std_1::primitive::IsntSliceExt,
    jay_config::{
        input::SwitchEvent,
        keyboard::{
            mods::{CAPS, Modifiers, NUM, RELEASE},
            syms::KeySym,
        },
    },
    kbvm::{ModifierMask, state_machine::Event},
    linearize::LinearizeExt,
    smallvec::SmallVec,
    std::{cell::RefCell, collections::hash_map::Entry, mem, rc::Rc},
};

#[derive(Default)]
pub struct NodeSeatState {
    pointer_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    kb_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    gesture_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    touch_foci: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    pointer_grabs: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    dnd_targets: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    tablet_pad_foci: SmallMap<TabletPadId, Rc<TabletPad>, 1>,
    tablet_tool_foci: SmallMap<TabletToolId, Rc<TabletTool>, 1>,
    ui_drags: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
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

    pub(super) fn add_ui_drag(&self, seat: &Rc<WlSeatGlobal>) {
        self.ui_drags.insert(seat.id, seat.clone());
    }

    pub(super) fn remove_ui_drag(&self, seat: &WlSeatGlobal) {
        self.ui_drags.remove(&seat.id);
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

    pub(super) fn touch_begin(&self, seat: &Rc<WlSeatGlobal>) {
        self.touch_foci.insert(seat.id, seat.clone());
    }

    pub(super) fn touch_end(&self, seat: &WlSeatGlobal) {
        self.touch_foci.remove(&seat.id);
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
            seat.kb_owner
                .set_kb_node(&seat, seat.state.root.clone(), seat.state.next_serial(None));
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

    pub fn destroy_node(&self, node: &dyn Node) {
        self.destroy_node2(node, true);
    }

    fn destroy_node2(&self, node: &dyn Node, focus_last: bool) {
        // NOTE: Also called by set_visible(false)

        while let Some((_, seat)) = self.gesture_foci.pop() {
            seat.gesture_owner.revert_to_default(&seat);
        }
        while let Some((_, seat)) = self.pointer_grabs.pop() {
            seat.pointer_owner.grab_node_removed(&seat);
        }
        while let Some((_, seat)) = self.ui_drags.pop() {
            seat.pointer_owner.revert_to_default(&seat);
        }
        let node_id = node.node_id();
        while let Some((_, seat)) = self.dnd_targets.pop() {
            seat.pointer_owner.dnd_target_removed(&seat);
        }
        while let Some((_, seat)) = self.pointer_foci.pop() {
            let mut ps = seat.pointer_stack.borrow_mut();
            while let Some(last) = ps.pop() {
                last.node_on_leave(&seat);
                if last.node_id() == node_id {
                    break;
                }
                last.node_seat_state().leave(&seat);
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
        while let Some((_, seat)) = self.touch_foci.pop() {
            seat.touch_owner.cancel(&seat);
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
            | InputEvent::TabletPadStrip { time_usec, .. }
            | InputEvent::TabletPadDial { time_usec, .. }
            | InputEvent::TouchFrame { time_usec, .. } => {
                self.last_input_usec.set(time_usec);
                if self.idle_notifications.is_not_empty() {
                    for notification in self.idle_notifications.lock().drain_values() {
                        notification.resume.trigger();
                    }
                }
            }
            InputEvent::AxisPx { .. }
            | InputEvent::AxisSource { .. }
            | InputEvent::AxisStop { .. }
            | InputEvent::Axis120 { .. }
            | InputEvent::TabletToolAdded { .. }
            | InputEvent::TabletToolRemoved { .. }
            | InputEvent::TouchDown { .. }
            | InputEvent::TouchUp { .. }
            | InputEvent::TouchMotion { .. }
            | InputEvent::TouchCancel { .. } => {}
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
            InputEvent::TabletPadDial { .. } => {}
            InputEvent::TouchDown { .. } => {}
            InputEvent::TouchUp { .. } => {}
            InputEvent::TouchMotion { .. } => {}
            InputEvent::TouchCancel { .. } => {}
            InputEvent::TouchFrame { .. } => {}
        }
        match event {
            InputEvent::Key {
                time_usec,
                key,
                state,
            } => {
                self.get_physical_keyboard(dev.keyboard_id, dev.keymap.get().as_ref())
                    .phy_state
                    .update(time_usec, self, key, state);
            }
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

            InputEvent::AxisSource { source } => self.axis_source(source),
            InputEvent::Axis120 {
                dist,
                axis,
                inverted,
            } => self.axis_120(dist, axis, inverted),
            InputEvent::AxisPx {
                dist,
                axis,
                inverted,
            } => self.axis_px(dist, axis, inverted),
            InputEvent::AxisStop { axis } => self.axis_stop(axis),
            InputEvent::AxisFrame { time_usec } => {
                self.axis_frame(dev.px_per_scroll_wheel.get(), time_usec)
            }
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
            InputEvent::TabletPadDial {
                time_usec,
                pad,
                dial,
                value120,
            } => self.tablet_event_pad_dial(pad, dial, value120, time_usec),
            InputEvent::TouchDown {
                time_usec,
                id,
                x_normed,
                y_normed,
            } => self.touch_down(time_usec, id, dev.get_rect(&self.state), x_normed, y_normed),
            InputEvent::TouchUp { time_usec, id } => self.touch_up(time_usec, id),
            InputEvent::TouchMotion {
                time_usec,
                id,
                x_normed,
                y_normed,
            } => self.touch_motion(time_usec, id, dev.get_rect(&self.state), x_normed, y_normed),
            InputEvent::TouchCancel { time_usec, id } => self.touch_cancel(time_usec, id),
            InputEvent::TouchFrame { time_usec } => self.touch_frame(time_usec),
        }
    }

    fn set_pointer_cursor_position(&self, x: Fixed, y: Fixed) -> (Fixed, Fixed) {
        let dnd_icon = self.pointer_owner.dnd_icon();
        if let Some(dnd_icon) = &dnd_icon {
            let (x_old, y_old) = self.pointer_cursor.position_int();
            dnd_icon.damage_at(x_old, y_old);
        }
        let (x, y) = self.pointer_cursor.set_position(x, y);
        let x_int = x.round_down();
        let y_int = y.round_down();
        if let Some(dnd_icon) = &dnd_icon {
            dnd_icon.damage_at(x_int, y_int);
        }
        if let Some(td) = self.pointer_owner.toplevel_drag() {
            td.move_(x_int, y_int);
        }
        (x, y)
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
        self.motion_event_abs(time_usec, x, y, false);
    }

    pub fn motion_event_abs(self: &Rc<Self>, time_usec: u64, x: Fixed, y: Fixed, defer: bool) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_motion_abs(time_usec, x, y);
        });
        let (x, y) = self.set_pointer_cursor_position(x, y);
        if let Some(c) = self.constraint.get() {
            if c.ty == ConstraintType::Lock || !c.contains(x.round_down(), y.round_down()) {
                c.deactivate(false);
            }
        }
        self.state.for_each_seat_tester(|t| {
            t.send_pointer_abs(self.id, time_usec, x, y);
        });
        self.cursor_moved(time_usec, defer);
    }

    pub fn motion_event(
        self: &Rc<Self>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_motion(time_usec, dx, dy);
        });
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
        self.set_pointer_cursor_position(x, y);
        self.cursor_moved(time_usec, false);
    }

    pub fn button_event(self: &Rc<Self>, time_usec: u64, button: u32, state: KeyState) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_button(time_usec, button, state);
        });
        self.state.for_each_seat_tester(|t| {
            t.send_button(self.id, time_usec, button, state);
        });
        self.pointer_owner.button(self, time_usec, button, state);
    }

    pub fn axis_source(&self, axis_source: AxisSource) {
        self.pointer_owner.axis_source(axis_source);
    }

    pub fn axis_120(&self, delta: i32, axis: ScrollAxis, inverted: bool) {
        self.pointer_owner.axis_120(delta, axis, inverted);
    }

    pub fn axis_px(&self, delta: Fixed, axis: ScrollAxis, inverted: bool) {
        self.pointer_owner.axis_px(delta, axis, inverted);
    }

    pub fn axis_stop(&self, axis: ScrollAxis) {
        self.pointer_owner.axis_stop(axis);
    }

    pub fn axis_frame(self: &Rc<Self>, px_per_scroll_wheel: f64, time_usec: u64) {
        self.pointer_owner
            .frame(px_per_scroll_wheel, self, time_usec);
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

    fn touch_down(
        self: &Rc<Self>,
        time_usec: u64,
        id: i32,
        rect: Rect,
        x_normed: Fixed,
        y_normed: Fixed,
    ) {
        let x = Fixed::from_f64(rect.x1() as f64 + rect.width() as f64 * x_normed.to_f64());
        let y = Fixed::from_f64(rect.y1() as f64 + rect.height() as f64 * y_normed.to_f64());
        self.touch_down_at(time_usec, id, x, y);
    }

    pub fn touch_down_at(self: &Rc<Self>, time_usec: u64, id: i32, x: Fixed, y: Fixed) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_touch_down(id as _, x, y);
        });
        self.cursor_group().deactivate();
        self.state.for_each_seat_tester(|t| {
            t.send_touch_down(self.id, time_usec, id, x, y);
        });
        self.touch_owner.down(self, time_usec, id, x, y);
    }

    pub fn touch_up(self: &Rc<Self>, time_usec: u64, id: i32) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_touch_up(id as _);
        });
        self.state.for_each_seat_tester(|t| {
            t.send_touch_up(self.id, time_usec, id);
        });
        self.touch_owner.up(self, time_usec, id);
    }

    fn touch_motion(
        self: &Rc<Self>,
        time_usec: u64,
        id: i32,
        rect: Rect,
        x_normed: Fixed,
        y_normed: Fixed,
    ) {
        let x = Fixed::from_f64(rect.x1() as f64 + rect.width() as f64 * x_normed.to_f64());
        let y = Fixed::from_f64(rect.y1() as f64 + rect.height() as f64 * y_normed.to_f64());
        self.touch_motion_at(time_usec, id, x, y);
    }

    pub fn touch_motion_at(self: &Rc<Self>, time_usec: u64, id: i32, x: Fixed, y: Fixed) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_touch_motion(id as _, x, y);
        });
        self.cursor_group().deactivate();
        self.state.for_each_seat_tester(|t| {
            t.send_touch_motion(self.id, time_usec, id, x, y);
        });
        self.touch_owner.motion(self, time_usec, id, x, y);
    }

    pub fn touch_cancel(self: &Rc<Self>, time_usec: u64, id: i32) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_touch_cancel(id as _);
        });
        self.state.for_each_seat_tester(|t| {
            t.send_touch_cancel(self.id, time_usec, id);
        });
        self.touch_owner.cancel(self);
    }

    pub fn touch_frame(self: &Rc<Self>, time_usec: u64) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_touch_frame(time_usec);
        });
        self.touch_owner.frame(self);
    }

    pub fn key_events(
        self: &Rc<Self>,
        time_usec: u64,
        events: &SyncQueue<Event>,
        kbvm_state_rc: &Rc<RefCell<KbvmState>>,
    ) {
        let mut kbvm_state = kbvm_state_rc.borrow_mut();
        self.latest_kb_state.set(kbvm_state_rc.clone());
        self.latest_kb_state_id.set(kbvm_state.kb_state.id);
        let mut shortcuts = SmallVec::<[_; 1]>::new();
        let mut components_changed = false;
        while let Some(event) = events.pop() {
            components_changed |= kbvm_state.kb_state.mods.apply_event(event);
            let (key_state, kc) = match event {
                Event::KeyDown(kc) => (KeyState::Pressed, kc),
                Event::KeyUp(kc) => (KeyState::Released, kc),
                _ => continue,
            };
            let update_pressed_keys = |kbvm_state: &mut KbvmState| {
                let pk = &mut kbvm_state.kb_state.pressed_keys;
                match key_state {
                    KeyState::Released => pk.remove(&kc.to_evdev()),
                    KeyState::Pressed => pk.insert(kc.to_evdev()),
                }
            };
            shortcuts.clear();
            {
                let mut mods = kbvm_state.kb_state.mods.mods.0 & !(CAPS.0 | NUM.0);
                if key_state == KeyState::Released {
                    mods |= RELEASE.0;
                }
                let scs = &*self.shortcuts.borrow();
                let keysyms = kbvm_state.map.lookup_table.lookup(
                    kbvm_state.kb_state.mods.group,
                    ModifierMask::default(),
                    kc,
                );
                let mut revert_pointer_to_default = false;
                for props in keysyms {
                    let sym = props.keysym().0;
                    if sym == self.revert_key.get().0 && mods == 0 {
                        revert_pointer_to_default = true;
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
                if revert_pointer_to_default {
                    drop(kbvm_state);
                    self.pointer_owner.revert_to_default(self);
                    kbvm_state = kbvm_state_rc.borrow_mut();
                }
            }
            self.state.for_each_seat_tester(|t| {
                t.send_key(self.id, time_usec, kc.to_evdev(), key_state);
            });
            if shortcuts.is_not_empty() {
                self.forward.set(key_state == KeyState::Released);
                if let Some(config) = self.state.config.get() {
                    drop(kbvm_state);
                    for shortcut in &shortcuts {
                        config.invoke_shortcut(self.id(), shortcut);
                    }
                    kbvm_state = kbvm_state_rc.borrow_mut();
                    if kbvm_state.kb_state.id != self.latest_kb_state_id.get() {
                        update_pressed_keys(&mut kbvm_state);
                        kbvm_state.apply_events(events);
                        return;
                    }
                }
                if !self.forward.get() {
                    update_pressed_keys(&mut kbvm_state);
                    continue;
                }
            }
            self.send_components(&mut components_changed, &kbvm_state);
            match self.input_method_grab.get() {
                Some(g) => g.on_key(time_usec, kc.to_evdev(), key_state, &kbvm_state.kb_state),
                _ => self.keyboard_node.get().node_on_key(
                    self,
                    time_usec,
                    kc.to_evdev(),
                    key_state,
                    &kbvm_state.kb_state,
                ),
            }
            self.for_each_ei_seat(|ei_seat| {
                ei_seat.handle_key(time_usec, kc.to_evdev(), key_state, &kbvm_state.kb_state);
            });
            update_pressed_keys(&mut kbvm_state);
        }
        self.send_components(&mut components_changed, &kbvm_state);
    }

    fn send_components(&self, components_changed: &mut bool, kbvm_state: &KbvmState) {
        if !mem::take(components_changed) {
            return;
        }
        let kb_state = &kbvm_state.kb_state;
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_modifiers_changed(kb_state);
        });
        self.state.for_each_seat_tester(|t| {
            t.send_modifiers(self.id, &kb_state.mods);
        });
        match self.input_method_grab.get() {
            Some(g) => g.on_modifiers(kb_state),
            _ => self.keyboard_node.get().node_on_mods(self, kb_state),
        }
    }

    pub(super) fn for_each_ei_seat(&self, mut f: impl FnMut(&Rc<EiSeat>)) {
        if self.ei_seats.is_not_empty() {
            for ei_seat in self.ei_seats.lock().values() {
                f(ei_seat);
            }
        }
    }
}

impl WlSeatGlobal {
    pub fn pointer_node(&self) -> Option<Rc<dyn Node>> {
        self.pointer_stack.borrow().last().cloned()
    }

    pub fn focus_toplevel(self: &Rc<Self>, n: Rc<dyn ToplevelNode>) {
        let node = match n.tl_focus_child() {
            Some(n) => n,
            _ => n,
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
        if self.keyboard_node.get().node_id() == node.node_id() {
            return;
        }
        let serial = self.state.next_serial(node.node_client().as_deref());
        self.focus_node_with_serial(node, serial);
    }

    pub fn focus_node_with_serial(self: &Rc<Self>, node: Rc<dyn Node>, serial: u64) {
        self.kb_owner.set_kb_node(self, node, serial);
    }

    pub(super) fn for_each_seat<C>(&self, ver: Version, client: ClientId, mut f: C)
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

    fn for_each_touch<C>(&self, ver: Version, client: ClientId, mut f: C)
    where
        C: FnMut(&Rc<WlTouch>),
    {
        self.for_each_seat(ver, client, |seat| {
            let touches = seat.touches.lock();
            for touch in touches.values() {
                f(touch);
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

    pub fn surface_touch_event<F>(&self, ver: Version, surface: &WlSurface, mut f: F)
    where
        F: FnMut(&Rc<WlTouch>),
    {
        let client = &surface.client;
        self.for_each_touch(ver, client.id, |p| {
            f(p);
        });
    }

    fn cursor_moved(self: &Rc<Self>, time_usec: u64, defer: bool) {
        self.pos_time_usec.set(time_usec);
        self.changes.or_assign(CHANGE_CURSOR_MOVED);
        if defer {
            self.trigger_tree_changed(false);
        } else {
            self.apply_changes();
        }
    }

    pub fn emulate_cursor_moved(&self) {
        self.changes.or_assign(CHANGE_CURSOR_MOVED);
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

    pub fn trigger_tree_changed(&self, needs_layout: bool) {
        // log::info!("trigger_tree_changed");
        if needs_layout {
            self.tree_changed_needs_layout.set(true);
        }
        self.tree_changed.trigger();
    }

    pub(super) fn apply_changes(self: &Rc<Self>) {
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
        serial: u64,
    ) {
        let (state, pressed) = match state {
            KeyState::Released => (wl_pointer::RELEASED, false),
            KeyState::Pressed => {
                surface.client.focus_stealing_serial.set(Some(serial));
                (wl_pointer::PRESSED, true)
            }
        };
        let time = (time_usec / 1000) as u32;
        self.surface_pointer_event(Version::ALL, surface, |p| {
            p.send_button(serial, time, button, state)
        });
        self.surface_pointer_frame(surface);
        if pressed {
            if let Some(node) = surface.get_focus_node() {
                self.focus_node_with_serial(node, serial);
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
            for i in ScrollAxis::variants() {
                let i = i as usize;
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
        n.client.last_enter_serial.set(Some(serial));
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
            constraint.deactivate(true);
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
        state: KeyState,
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

// Touch callbacks
impl WlSeatGlobal {
    pub fn touch_down_surface(
        self: &Rc<Self>,
        surface: &WlSurface,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        let serial = surface.client.next_serial();
        surface.client.focus_stealing_serial.set(Some(serial));
        let time = (time_usec / 1000) as _;
        self.surface_touch_event(Version::ALL, surface, |t| {
            t.send_down(serial, time, surface.id, id, x, y)
        });
        if let Some(node) = surface.get_focus_node() {
            self.focus_node_with_serial(node, serial);
        }
    }

    pub fn touch_up_surface(&self, surface: &WlSurface, time_usec: u64, id: i32) {
        let serial = surface.client.next_serial();
        let time = (time_usec / 1000) as _;
        self.surface_touch_event(Version::ALL, surface, |t| t.send_up(serial, time, id))
    }

    pub fn touch_motion_surface(
        &self,
        surface: &WlSurface,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        let time = (time_usec / 1000) as _;
        self.surface_touch_event(Version::ALL, surface, |t| t.send_motion(time, id, x, y));
    }

    pub fn touch_frame_surface(&self, surface: &WlSurface) {
        self.surface_touch_event(Version::ALL, surface, |t| t.send_frame())
    }

    pub fn touch_cancel_surface(&self, surface: &WlSurface) {
        self.surface_touch_event(Version::ALL, surface, |t| t.send_cancel())
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
        serial: u64,
    ) {
        if let Some(src) = &dnd.src {
            if !surface.client.is_xwayland {
                offer_source_to_regular_client::<ClipboardIpc>(src.clone(), &surface.client);
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
