use {
    crate::{
        backend::{InputDeviceId, KeyState},
        client::Client,
        fixed::Fixed,
        ifs::wl_seat::{
            tablet::{
                PadButtonState, TabletRingEventSource, TabletStripEventSource, TabletTool,
                TabletToolChanges, TabletToolId, ToolButtonState,
            },
            wl_pointer::PendingScroll,
            SeatId,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{jay_seat_events::*, JaySeatEventsId},
        xkbcommon::ModifierState,
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct JaySeatEvents {
    pub id: JaySeatEventsId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JaySeatEvents {
    pub fn send_modifiers(&self, seat: SeatId, mods: &ModifierState) {
        self.client.event(Modifiers {
            self_id: self.id,
            seat: seat.raw(),
            modifiers: mods.mods_effective,
            group: mods.group,
        });
    }

    pub fn send_key(&self, seat: SeatId, time_usec: u64, key: u32, state: KeyState) {
        self.client.event(Key {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            key,
            state: state as u32,
        });
    }

    pub fn send_pointer_abs(&self, seat: SeatId, time_usec: u64, x: Fixed, y: Fixed) {
        self.client.event(PointerAbs {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            x,
            y,
        });
    }

    pub fn send_pointer_rel(
        &self,
        seat: SeatId,
        time_usec: u64,
        x: Fixed,
        y: Fixed,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        self.client.event(PointerRel {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            x,
            y,
            dx,
            dy,
            dx_unaccelerated,
            dy_unaccelerated,
        });
    }

    pub fn send_button(&self, seat: SeatId, time_usec: u64, button: u32, state: KeyState) {
        self.client.event(Button {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            button,
            state: state as u32,
        });
    }

    pub fn send_axis(&self, seat: SeatId, time_usec: u64, ps: &PendingScroll) {
        if let Some(source) = ps.source.get() {
            self.client.event(AxisSource {
                self_id: self.id,
                source,
            });
        }
        for axis in 0..1 {
            if let Some(dist) = ps.v120[axis].get() {
                self.client.event(Axis120 {
                    self_id: self.id,
                    dist,
                    axis: axis as _,
                });
            }
            if let Some(dist) = ps.px[axis].get() {
                self.client.event(AxisInverted {
                    self_id: self.id,
                    axis: axis as _,
                    inverted: ps.inverted[axis].get() as _,
                });
                self.client.event(AxisPx {
                    self_id: self.id,
                    dist,
                    axis: axis as _,
                });
            }
            if ps.stop[axis].get() {
                self.client.event(AxisStop {
                    self_id: self.id,
                    axis: axis as _,
                });
            }
        }
        self.client.event(AxisFrame {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
        });
    }

    pub fn send_swipe_begin(&self, seat: SeatId, time_usec: u64, finger_count: u32) {
        self.client.event(SwipeBegin {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            fingers: finger_count,
        });
    }

    pub fn send_swipe_update(
        &self,
        seat: SeatId,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        self.client.event(SwipeUpdate {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            dx,
            dy,
            dx_unaccelerated,
            dy_unaccelerated,
        });
    }

    pub fn send_swipe_end(&self, seat: SeatId, time_usec: u64, cancelled: bool) {
        self.client.event(SwipeEnd {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            cancelled: cancelled as _,
        });
    }

    pub fn send_pinch_begin(&self, seat: SeatId, time_usec: u64, finger_count: u32) {
        self.client.event(PinchBegin {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            fingers: finger_count,
        });
    }

    pub fn send_pinch_update(
        &self,
        seat: SeatId,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        self.client.event(PinchUpdate {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            dx,
            dy,
            dx_unaccelerated,
            dy_unaccelerated,
            scale,
            rotation,
        });
    }

    pub fn send_pinch_end(&self, seat: SeatId, time_usec: u64, cancelled: bool) {
        self.client.event(PinchEnd {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            cancelled: cancelled as _,
        });
    }

    pub fn send_hold_begin(&self, seat: SeatId, time_usec: u64, finger_count: u32) {
        self.client.event(HoldBegin {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            fingers: finger_count,
        });
    }

    pub fn send_hold_end(&self, seat: SeatId, time_usec: u64, cancelled: bool) {
        self.client.event(HoldEnd {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            cancelled: cancelled as _,
        });
    }

    pub fn send_switch_event(
        &self,
        seat: SeatId,
        input_device: InputDeviceId,
        time_usec: u64,
        event: jay_config::input::SwitchEvent,
    ) {
        self.client.event(SwitchEvent {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: input_device.raw(),
            event: event as _,
        });
    }

    pub fn send_tablet_tool_proximity_in(
        &self,
        seat: SeatId,
        tablet: InputDeviceId,
        tool: TabletToolId,
        time_usec: u64,
    ) {
        self.client
            .event(TabletToolProximityIn { self_id: self.id });
        self.client.event(TabletToolFrame {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: tablet.raw(),
            tool: tool.raw() as _,
        });
    }

    pub fn send_tablet_tool_proximity_out(
        &self,
        seat: SeatId,
        tablet: InputDeviceId,
        tool: TabletToolId,
        time_usec: u64,
    ) {
        self.client
            .event(TabletToolProximityOut { self_id: self.id });
        self.client.event(TabletToolFrame {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: tablet.raw(),
            tool: tool.raw() as _,
        });
    }

    pub fn send_tablet_tool_changes(
        &self,
        seat: SeatId,
        tablet: InputDeviceId,
        tool: &TabletTool,
        time_usec: u64,
        changes: &TabletToolChanges,
    ) {
        let self_id = self.id;
        if let Some(down) = changes.down {
            match down {
                true => self.client.event(TabletToolDown { self_id }),
                false => self.client.event(TabletToolUp { self_id }),
            }
        }
        if changes.pos.is_some() {
            let (x, y) = tool.cursor().position();
            self.client.event(TabletToolMotion { self_id, x, y });
        }
        if let Some(val) = changes.pressure {
            self.client.event(TabletToolPressure {
                self_id,
                pressure: val,
            });
        }
        if let Some(val) = changes.distance {
            self.client.event(TabletToolDistance {
                self_id,
                distance: val,
            });
        }
        if let Some(val) = changes.tilt {
            self.client.event(TabletToolTilt {
                self_id,
                tilt_x: val.x,
                tilt_y: val.y,
            });
        }
        if let Some(val) = changes.rotation {
            self.client.event(TabletToolRotation {
                self_id,
                degrees: val,
            });
        }
        if let Some(val) = changes.slider {
            self.client.event(TabletToolSlider {
                self_id,
                position: val,
            });
        }
        if let Some(val) = changes.wheel {
            self.client.event(TabletToolWheel {
                self_id,
                degrees: val.degrees,
                clicks: val.clicks,
            });
        }
        self.client.event(TabletToolFrame {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: tablet.raw(),
            tool: tool.id.raw() as _,
        });
    }

    pub fn send_tablet_tool_button(
        &self,
        seat: SeatId,
        tablet: InputDeviceId,
        tool: &TabletTool,
        time_usec: u64,
        button: u32,
        state: ToolButtonState,
    ) {
        self.client.event(TabletToolButton {
            self_id: self.id,
            button,
            state: state as _,
        });
        self.client.event(TabletToolFrame {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: tablet.raw(),
            tool: tool.id.raw() as _,
        });
    }

    pub fn send_tablet_pad_mode_switch(
        &self,
        seat: SeatId,
        pad: InputDeviceId,
        time_usec: u64,
        group: u32,
        mode: u32,
    ) {
        self.client.event(TabletPadModeSwitch {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: pad.raw(),
            group,
            mode,
        });
    }

    pub fn send_tablet_pad_button(
        &self,
        seat: SeatId,
        pad: InputDeviceId,
        time_usec: u64,
        button: u32,
        state: PadButtonState,
    ) {
        self.client.event(TabletPadButton {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: pad.raw(),
            button,
            state: state as _,
        });
    }

    pub fn send_tablet_pad_strip(
        &self,
        seat: SeatId,
        pad: InputDeviceId,
        time_usec: u64,
        strip: u32,
        source: Option<TabletStripEventSource>,
        position: Option<f64>,
    ) {
        if let Some(source) = source {
            self.client.event(TabletPadStripSource {
                self_id: self.id,
                source: source as _,
            });
        }
        if let Some(position) = position {
            self.client.event(TabletPadStripPosition {
                self_id: self.id,
                position,
            });
        } else {
            self.client.event(TabletPadStripStop { self_id: self.id });
        }
        self.client.event(TabletPadStripFrame {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: pad.raw(),
            strip,
        });
    }

    pub fn send_tablet_pad_ring(
        &self,
        seat: SeatId,
        pad: InputDeviceId,
        time_usec: u64,
        ring: u32,
        source: Option<TabletRingEventSource>,
        degrees: Option<f64>,
    ) {
        if let Some(source) = source {
            self.client.event(TabletPadRingSource {
                self_id: self.id,
                source: source as _,
            });
        }
        if let Some(degrees) = degrees {
            self.client.event(TabletPadRingAngle {
                self_id: self.id,
                degrees,
            });
        } else {
            self.client.event(TabletPadRingStop { self_id: self.id });
        }
        self.client.event(TabletPadRingFrame {
            self_id: self.id,
            seat: seat.raw(),
            time_usec,
            input_device: pad.raw(),
            ring,
        });
    }
}

impl JaySeatEventsRequestHandler for JaySeatEvents {
    type Error = Infallible;
}

object_base! {
    self = JaySeatEvents;
    version = Version(1);
}

impl Object for JaySeatEvents {
    fn break_loops(&self) {
        self.client
            .state
            .testers
            .borrow_mut()
            .remove(&(self.client.id, self.id));
    }
}

simple_add_obj!(JaySeatEvents);
