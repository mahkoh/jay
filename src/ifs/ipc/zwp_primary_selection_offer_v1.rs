use {
    crate::{
        client::{Client, ClientError, ClientId},
        ifs::{
            ipc::{
                break_offer_loops, cancel_offer, destroy_data_offer, receive_data_offer,
                zwp_primary_selection_device_v1::{
                    PrimarySelectionIpc, ZwpPrimarySelectionDeviceV1,
                },
                DataOffer, DataOfferId, DynDataOffer, OfferData,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_primary_selection_offer_v1::*, ZwpPrimarySelectionOfferV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

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
        destroy_data_offer::<PrimarySelectionIpc>(self);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpPrimarySelectionOfferV1;
    version = self.version;
}

impl Object for ZwpPrimarySelectionOfferV1 {
    fn break_loops(&self) {
        break_offer_loops::<PrimarySelectionIpc>(self);
    }
}

simple_add_obj!(ZwpPrimarySelectionOfferV1);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionOfferV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPrimarySelectionOfferV1Error, ClientError);
