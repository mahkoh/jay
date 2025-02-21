use {
    crate::{
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::{wl_seat::WlSeatGlobal, wl_surface::WlSurface},
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpPointerGesturePinchV1Id, zwp_pointer_gesture_pinch_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpPointerGesturePinchV1 {
    pub id: ZwpPointerGesturePinchV1Id,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeatGlobal>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpPointerGesturePinchV1 {
    fn detach(&self) {
        self.seat.pinch_bindings.remove(&self.client, self);
    }

    pub fn send_pinch_begin(&self, n: &WlSurface, serial: u64, time_usec: u64, finger_count: u32) {
        self.client.event(Begin {
            self_id: self.id,
            serial: serial as _,
            time: (time_usec / 1000) as u32,
            surface: n.id,
            fingers: finger_count,
        });
    }

    pub fn send_pinch_update(
        &self,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        self.client.event(Update {
            self_id: self.id,
            time: (time_usec / 1000) as u32,
            dx,
            dy,
            scale,
            rotation,
        });
    }

    pub fn send_pinch_end(&self, serial: u64, time_usec: u64, cancelled: bool) {
        self.client.event(End {
            self_id: self.id,
            serial: serial as _,
            time: (time_usec / 1000) as u32,
            cancelled: cancelled as _,
        });
    }
}

impl ZwpPointerGesturePinchV1RequestHandler for ZwpPointerGesturePinchV1 {
    type Error = ZwpPointerGesturePinchV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpPointerGesturePinchV1;
    version = self.version;
}

impl Object for ZwpPointerGesturePinchV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpPointerGesturePinchV1);

#[derive(Debug, Error)]
pub enum ZwpPointerGesturePinchV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPointerGesturePinchV1Error, ClientError);
