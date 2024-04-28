use {
    crate::{
        backend::{InputDeviceId, KeyState},
        client::Client,
        fixed::Fixed,
        ifs::wl_seat::{wl_pointer::PendingScroll, SeatId},
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
