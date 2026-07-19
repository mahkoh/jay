use crate::client::Client;
use crate::client::ClientError;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::ZwpPointerGestureHoldV1Id;
use crate::wire::zwp_pointer_gesture_hold_v1::*;
use std::rc::Rc;
use thiserror::Error;

pub struct ZwpPointerGestureHoldV1 {
    pub id: ZwpPointerGestureHoldV1Id,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeatGlobal>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpPointerGestureHoldV1 {
    fn detach(&self) {
        self.seat.hold_bindings.remove(&self.client, self);
    }

    pub fn send_hold_begin(&self, n: &WlSurface, serial: u64, time_usec: u64, finger_count: u32) {
        self.client.event(Begin {
            self_id: self.id,
            serial: serial as _,
            time: (time_usec / 1000) as u32,
            surface: n.id,
            fingers: finger_count,
        });
    }

    pub fn send_hold_end(&self, serial: u64, time_usec: u64, cancelled: bool) {
        self.client.event(End {
            self_id: self.id,
            serial: serial as _,
            time: (time_usec / 1000) as u32,
            cancelled: cancelled as _,
        });
    }
}

impl ZwpPointerGestureHoldV1RequestHandler for ZwpPointerGestureHoldV1 {
    type Error = ZwpPointerGestureHoldV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpPointerGestureHoldV1;
    version = self.version;
}

impl Object for ZwpPointerGestureHoldV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(ZwpPointerGestureHoldV1);

#[derive(Debug, Error)]
pub enum ZwpPointerGestureHoldV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPointerGestureHoldV1Error, ClientError);
