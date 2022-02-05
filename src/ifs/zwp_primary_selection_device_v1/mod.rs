mod types;

use crate::client::DynEventFormatter;
use crate::ifs::wl_seat::WlSeat;
use crate::ifs::zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1;
use crate::ifs::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1Id;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const SET_SELECTION: u32 = 0;
const DESTROY: u32 = 1;

const DATA_OFFER: u32 = 0;
const SELECTION: u32 = 1;

id!(ZwpPrimarySelectionDeviceV1Id);

pub struct ZwpPrimarySelectionDeviceV1 {
    pub id: ZwpPrimarySelectionDeviceV1Id,
    pub manager: Rc<ZwpPrimarySelectionDeviceManagerV1>,
    seat: Rc<WlSeat>,
}

impl ZwpPrimarySelectionDeviceV1 {
    pub fn new(
        id: ZwpPrimarySelectionDeviceV1Id,
        manager: &Rc<ZwpPrimarySelectionDeviceManagerV1>,
        seat: &Rc<WlSeat>,
    ) -> Self {
        Self {
            id,
            manager: manager.clone(),
            seat: seat.clone(),
        }
    }

    pub fn data_offer(self: &Rc<Self>, id: ZwpPrimarySelectionOfferV1Id) -> DynEventFormatter {
        Box::new(DataOffer {
            obj: self.clone(),
            id,
        })
    }

    pub fn selection(self: &Rc<Self>, id: ZwpPrimarySelectionOfferV1Id) -> DynEventFormatter {
        Box::new(Selection {
            obj: self.clone(),
            id,
        })
    }

    fn set_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSelectionError> {
        let req: SetSelection = self.manager.client.parse(self, parser)?;
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.manager.client.lookup(req.source)?)
        };
        self.seat.global.set_primary_selection(src)?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.manager.client.parse(self, parser)?;
        self.seat.remove_primary_selection_device(self);
        self.manager.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    ZwpPrimarySelectionDeviceV1, ZwpPrimarySelectionDeviceV1Error;

    SET_SELECTION => set_selection,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionDeviceV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.seat.remove_primary_selection_device(self);
    }
}

simple_add_obj!(ZwpPrimarySelectionDeviceV1);
