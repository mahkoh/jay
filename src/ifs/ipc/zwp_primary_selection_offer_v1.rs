use crate::client::Client;
use crate::client::ClientError;
use crate::client::ClientId;
use crate::ifs::ipc::DataOffer;
use crate::ifs::ipc::DataOfferId;
use crate::ifs::ipc::DynDataOffer;
use crate::ifs::ipc::OfferData;
use crate::ifs::ipc::OfferDestroyReason;
use crate::ifs::ipc::break_offer_loops;
use crate::ifs::ipc::cancel_offer;
use crate::ifs::ipc::destroy_data_offer_with_reason;
use crate::ifs::ipc::receive_data_offer;
use crate::ifs::ipc::zwp_primary_selection_device_v1::PrimarySelectionIpc;
use crate::ifs::ipc::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::ZwpPrimarySelectionOfferV1Id;
use crate::wire::zwp_primary_selection_offer_v1::*;
use std::rc::Rc;
use thiserror::Error;

pub struct ZwpPrimarySelectionOfferV1 {
    pub id: ZwpPrimarySelectionOfferV1Id,
    pub offer_id: DataOfferId,
    pub seat: Rc<WlSeatGlobal>,
    pub client: Rc<Client>,
    pub data: OfferData<ZwpPrimarySelectionDeviceV1>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl DataOffer for ZwpPrimarySelectionOfferV1 {
    type Device = ZwpPrimarySelectionDeviceV1;

    fn offer_data(&self) -> &OfferData<Self::Device> {
        &self.data
    }
}

impl DynDataOffer for ZwpPrimarySelectionOfferV1 {
    fn offer_id(&self) -> DataOfferId {
        self.offer_id
    }

    fn client_id(&self) -> ClientId {
        self.client.id
    }

    fn send_offer(&self, mime_type: &str) {
        ZwpPrimarySelectionOfferV1::send_offer(self, mime_type);
    }

    fn cancel(&self) {
        cancel_offer::<PrimarySelectionIpc>(self);
    }

    fn get_seat(&self) -> Rc<WlSeatGlobal> {
        self.seat.clone()
    }
}

impl ZwpPrimarySelectionOfferV1 {
    pub fn send_offer(&self, mime_type: &str) {
        self.client.event(Offer {
            self_id: self.id,
            mime_type,
        })
    }
}

impl ZwpPrimarySelectionOfferV1RequestHandler for ZwpPrimarySelectionOfferV1 {
    type Error = ZwpPrimarySelectionOfferV1Error;

    fn receive(&self, req: Receive, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        receive_data_offer::<PrimarySelectionIpc>(self, req.mime_type, req.fd);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        destroy_data_offer_with_reason::<PrimarySelectionIpc>(
            self,
            OfferDestroyReason::OfferClient,
        );
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpPrimarySelectionOfferV1;
    version = self.version;
}

impl Object for ZwpPrimarySelectionOfferV1 {
    fn break_loops(self: Rc<Self>) {
        break_offer_loops::<PrimarySelectionIpc>(&*self);
    }
}

simple_add_obj!(ZwpPrimarySelectionOfferV1);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionOfferV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPrimarySelectionOfferV1Error, ClientError);
