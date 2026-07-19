use crate::ifs::ipc::data_control::private::DataControlOffer;
use crate::ifs::ipc::data_control::private::DataControlOfferData;
use crate::ifs::ipc::data_control::private::logic::DataControlError;
use crate::ifs::ipc::data_control::private::logic::{self};
use crate::ifs::ipc::data_control::zwlr_data_control_device_v1::WlrDataControlIpc;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::wire::ZwlrDataControlOfferV1Id;
use crate::wire::zwlr_data_control_offer_v1::*;
use std::rc::Rc;
use thiserror::Error;

pub struct ZwlrDataControlOfferV1 {
    pub id: ZwlrDataControlOfferV1Id,
    pub data: DataControlOfferData<WlrDataControlIpc>,
    pub tracker: Tracker<Self>,
}

impl DataControlOffer for ZwlrDataControlOfferV1 {
    type Ipc = WlrDataControlIpc;

    fn data(&self) -> &DataControlOfferData<Self::Ipc> {
        &self.data
    }

    fn send_offer(&self, mime_type: &str) {
        self.send_offer(mime_type);
    }
}

impl ZwlrDataControlOfferV1 {
    pub fn send_offer(&self, mime_type: &str) {
        self.data.client.event(Offer {
            self_id: self.id,
            mime_type,
        })
    }
}

impl ZwlrDataControlOfferV1RequestHandler for ZwlrDataControlOfferV1 {
    type Error = ZwlrDataControlOfferV1Error;

    fn receive(&self, req: Receive, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        logic::data_offer_receive(self, req.mime_type, req.fd);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        logic::data_offer_destroy(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrDataControlOfferV1;
    version = self.data.device.data.version;
}

impl Object for ZwlrDataControlOfferV1 {
    fn break_loops(self: Rc<Self>) {
        logic::data_offer_break_loops(&*self);
    }
}

simple_add_obj!(ZwlrDataControlOfferV1);

#[derive(Debug, Error)]
pub enum ZwlrDataControlOfferV1Error {
    #[error(transparent)]
    DataControlError(#[from] DataControlError),
}
