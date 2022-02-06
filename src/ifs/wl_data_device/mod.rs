mod types;

use crate::client::DynEventFormatter;
use crate::ifs::wl_data_device_manager::WlDataDeviceManager;
use crate::ifs::wl_data_offer::WlDataOfferId;
use crate::ifs::wl_seat::WlSeat;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;
use crate::wire::wl_data_device::*;

#[allow(dead_code)]
const ROLE: u32 = 0;

pub struct WlDataDevice {
    pub id: WlDataDeviceId,
    pub manager: Rc<WlDataDeviceManager>,
    seat: Rc<WlSeat>,
}

impl WlDataDevice {
    pub fn new(id: WlDataDeviceId, manager: &Rc<WlDataDeviceManager>, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            manager: manager.clone(),
            seat: seat.clone(),
        }
    }

    pub fn data_offer(self: &Rc<Self>, id: WlDataOfferId) -> DynEventFormatter {
        Box::new(DataOffer {
            self_id: self.id,
            id,
        })
    }

    pub fn selection(self: &Rc<Self>, id: WlDataOfferId) -> DynEventFormatter {
        Box::new(Selection {
            self_id: self.id,
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
            Some(self.manager.client.lookup(req.source)?)
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
}

object_base! {
    WlDataDevice, WlDataDeviceError;

    START_DRAG => start_drag,
    SET_SELECTION => set_selection,
    RELEASE => release,
}

impl Object for WlDataDevice {
    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }

    fn break_loops(&self) {
        self.seat.remove_data_device(self);
    }
}

simple_add_obj!(WlDataDevice);
