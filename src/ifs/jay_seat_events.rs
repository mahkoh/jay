use {
    crate::{
        backend::KeyState,
        client::Client,
        fixed::Fixed,
        ifs::wl_seat::{wl_pointer::PendingScroll, SeatId},
        leaks::Tracker,
        object::Object,
        wire::{jay_seat_events::*, JaySeatEventsId},
        xkbcommon::ModifierState,
    },
    std::rc::Rc,
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
}

object_base! {
    self = JaySeatEvents;
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
