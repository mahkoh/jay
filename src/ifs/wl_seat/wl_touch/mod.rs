mod types;

use crate::ifs::wl_seat::WlSeat;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const RELEASE: u32 = 0;

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

id!(WlTouchId);

pub struct WlTouch {
    id: WlTouchId,
    seat: Rc<WlSeat>,
}

impl WlTouch {
    pub fn new(id: WlTouchId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            seat: seat.clone(),
        }
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.seat.client.parse(self, parser)?;
        self.seat.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    WlTouch, WlTouchError;

    RELEASE => release,
}

impl Object for WlTouch {
    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }
}

simple_add_obj!(WlTouch);
