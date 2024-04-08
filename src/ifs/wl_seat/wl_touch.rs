use {
    crate::{
        client::ClientError,
        ifs::wl_seat::WlSeat,
        leaks::Tracker,
        object::Object,
        wire::{wl_touch::*, WlTouchId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[allow(dead_code)]
const DOWN: u32 = 0;
#[allow(dead_code)]
const UP: u32 = 1;
#[allow(dead_code)]
const MOTION: u32 = 2;
#[allow(dead_code)]
const FRAME: u32 = 3;
#[allow(dead_code)]
const CANCEL: u32 = 4;
#[allow(dead_code)]
const SHAPE: u32 = 5;
#[allow(dead_code)]
const ORIENTATION: u32 = 6;

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
}

impl WlTouchRequestHandler for WlTouch {
    type Error = WlTouchError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
