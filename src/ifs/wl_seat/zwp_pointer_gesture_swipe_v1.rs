use {
    crate::{
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::{wl_seat::WlSeatGlobal, wl_surface::WlSurface},
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_pointer_gesture_swipe_v1::*, ZwpPointerGestureSwipeV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpPointerGestureSwipeV1 {
    pub id: ZwpPointerGestureSwipeV1Id,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeatGlobal>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpPointerGestureSwipeV1 {
    fn detach(&self) {
        self.seat.swipe_bindings.remove(&self.client, self);
    }

    pub fn send_swipe_begin(&self, n: &WlSurface, serial: u64, time_usec: u64, finger_count: u32) {
        self.client.event(Begin {
            self_id: self.id,
            serial: serial as _,
            time: (time_usec / 1000) as u32,
            surface: n.id,
            fingers: finger_count,
        });
    }

    pub fn send_swipe_update(&self, time_usec: u64, dx: Fixed, dy: Fixed) {
        self.client.event(Update {
            self_id: self.id,
            time: (time_usec / 1000) as u32,
            dx,
            dy,
        });
    }

    pub fn send_swipe_end(&self, serial: u64, time_usec: u64, cancelled: bool) {
        self.client.event(End {
            self_id: self.id,
            serial: serial as _,
            time: (time_usec / 1000) as u32,
            cancelled: cancelled as _,
        });
    }
}

impl ZwpPointerGestureSwipeV1RequestHandler for ZwpPointerGestureSwipeV1 {
    type Error = ZwpPointerGestureSwipeV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpPointerGestureSwipeV1;
    version = self.version;
}

impl Object for ZwpPointerGestureSwipeV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpPointerGestureSwipeV1);

#[derive(Debug, Error)]
pub enum ZwpPointerGestureSwipeV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPointerGestureSwipeV1Error, ClientError);
