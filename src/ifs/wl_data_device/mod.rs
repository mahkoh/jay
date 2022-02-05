mod types;

use crate::client::{DynEventFormatter};
use crate::ifs::wl_data_device_manager::WlDataDeviceManagerObj;
use crate::ifs::wl_data_offer::WlDataOfferId;
use crate::ifs::wl_seat::{WlSeatObj};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const START_DRAG: u32 = 0;
const SET_SELECTION: u32 = 1;
const RELEASE: u32 = 2;

const DATA_OFFER: u32 = 0;
const ENTER: u32 = 1;
const LEAVE: u32 = 2;
const MOTION: u32 = 4;
const DROP: u32 = 5;
const SELECTION: u32 = 5;

#[allow(dead_code)]
const ROLE: u32 = 0;

id!(WlDataDeviceId);

pub struct WlDataDevice {
    pub id: WlDataDeviceId,
    pub manager: Rc<WlDataDeviceManagerObj>,
    seat: Rc<WlSeatObj>,
}

impl WlDataDevice {
    pub fn new(
        id: WlDataDeviceId,
        manager: &Rc<WlDataDeviceManagerObj>,
        seat: &Rc<WlSeatObj>,
    ) -> Self {
        Self {
            id,
            manager: manager.clone(),
            seat: seat.clone(),
        }
    }

    pub fn data_offer(self: &Rc<Self>, id: WlDataOfferId) -> DynEventFormatter {
        Box::new(DataOffer {
            obj: self.clone(),
            id,
        })
    }

    pub fn selection(self: &Rc<Self>, id: WlDataOfferId) -> DynEventFormatter {
        Box::new(Selection {
            obj: self.clone(),
            id,
        })
    }

    fn start_drag(&self, parser: MsgParser<'_, '_>) -> Result<(), StartDragError> {
        let _req: StartDrag = self.manager.client.parse(self, parser)?;
        Ok(())
    }

    fn set_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSelectionError> {
        let req: SetSelection = self.manager.client.parse(self, parser)?;
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.manager.client.get_wl_data_source(req.source)?)
        };
        self.seat.global.set_selection(src)?;
        Ok(())
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.manager.client.parse(self, parser)?;
        self.seat.remove_data_device(self);
        self.manager.client.remove_obj(self)?;
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDataDeviceError> {
        match request {
            START_DRAG => self.start_drag(parser)?,
            SET_SELECTION => self.set_selection(parser)?,
            RELEASE => self.release(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlDataDevice);

impl Object for WlDataDevice {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlDataDevice
    }

    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }

    fn break_loops(&self) {
        self.seat.remove_data_device(self);
    }
}
