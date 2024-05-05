use {
    crate::{
        client::ClientError,
        fixed::Fixed,
        ifs::wl_seat::WlSeat,
        leaks::Tracker,
        object::{Object, Version},
        wire::{wl_touch::*, WlSurfaceId, WlTouchId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[allow(dead_code)]
pub const SHAPE_SINCE_VERSION: Version = Version(6);
#[allow(dead_code)]
pub const ORIENTATION_DIRECTION_SINCE_VERSION: Version = Version(6);

pub struct WlTouch {
    id: WlTouchId,
    seat: Rc<WlSeat>,
    pub tracker: Tracker<Self>,
}

impl WlTouch {
    pub fn new(id: WlTouchId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            seat: seat.clone(),
            tracker: Default::default(),
        }
    }

    pub fn send_down(
        &self,
        serial: u32,
        time: u32,
        surface: WlSurfaceId,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        self.seat.client.event(Down {
            self_id: self.id,
            serial,
            time,
            surface,
            id,
            x,
            y,
        })
    }

    pub fn send_up(&self, serial: u32, time: u32, id: i32) {
        self.seat.client.event(Up {
            self_id: self.id,
            serial,
            time,
            id,
        })
    }

    pub fn send_motion(&self, time: u32, id: i32, x: Fixed, y: Fixed) {
        self.seat.client.event(Motion {
            self_id: self.id,
            time,
            id,
            x,
            y,
        })
    }

    pub fn send_frame(&self) {
        self.seat.client.event(Frame { self_id: self.id })
    }

    pub fn send_cancel(&self) {
        self.seat.client.event(Cancel { self_id: self.id })
    }

    #[allow(dead_code)]
    pub fn send_shape(&self, id: i32, major: Fixed, minor: Fixed) {
        self.seat.client.event(Shape {
            self_id: self.id,
            id,
            major,
            minor,
        })
    }

    #[allow(dead_code)]
    pub fn send_orientation(&self, id: i32, orientation: Fixed) {
        self.seat.client.event(Orientation {
            self_id: self.id,
            id,
            orientation,
        })
    }
}

impl WlTouchRequestHandler for WlTouch {
    type Error = WlTouchError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.seat.touches.remove(&self.id);
        self.seat.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WlTouch;
    version = self.seat.version;
}

impl Object for WlTouch {}

simple_add_obj!(WlTouch);

#[derive(Debug, Error)]
pub enum WlTouchError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlTouchError, ClientError);
