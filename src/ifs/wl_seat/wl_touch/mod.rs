mod types;

use crate::client::{AddObj};
use crate::ifs::wl_seat::WlSeatObj;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const RELEASE: u32 = 0;

const DOWN: u32 = 0;
const UP: u32 = 1;
const MOTION: u32 = 2;
const FRAME: u32 = 3;
const CANCEL: u32 = 4;
const SHAPE: u32 = 5;
const ORIENTATION: u32 = 6;

id!(WlTouchId);

pub struct WlTouch {
    id: WlTouchId,
    seat: Rc<WlSeatObj>,
}

impl WlTouch {
    pub fn new(id: WlTouchId, seat: &Rc<WlSeatObj>) -> Self {
        Self {
            id,
            seat: seat.clone(),
        }
    }

    async fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.seat.client.parse(self, parser)?;
        self.seat.client.remove_obj(self).await?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlTouchError> {
        match request {
            RELEASE => self.release(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlTouch);

impl Object for WlTouch {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlTouch
    }

    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }
}
