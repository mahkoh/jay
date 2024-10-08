use {
    crate::{
        client::{Client, ClientError, ClientId},
        ifs::{
            ipc::{
                break_offer_loops, cancel_offer,
                data_control::zwlr_data_control_device_v1::{
                    WlrClipboardIpc, WlrPrimarySelectionIpc, ZwlrDataControlDeviceV1,
                },
                destroy_data_offer, receive_data_offer, DataOffer, DataOfferId, DynDataOffer,
                IpcLocation, OfferData,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::Object,
        wire::{zwlr_data_control_offer_v1::*, ZwlrDataControlOfferV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwlrDataControlOfferV1 {
    pub id: ZwlrDataControlOfferV1Id,
    pub offer_id: DataOfferId,
    pub client: Rc<Client>,
    pub device: Rc<ZwlrDataControlDeviceV1>,
    pub data: OfferData<ZwlrDataControlDeviceV1>,
    pub location: IpcLocation,
    pub tracker: Tracker<Self>,
}

impl DataOffer for ZwlrDataControlOfferV1 {
    type Device = ZwlrDataControlDeviceV1;

    fn offer_data(&self) -> &OfferData<ZwlrDataControlDeviceV1> {
        &self.data
    }
}

impl DynDataOffer for ZwlrDataControlOfferV1 {
    fn offer_id(&self) -> DataOfferId {
        self.offer_id
    }

    fn client_id(&self) -> ClientId {
        self.client.id
    }

    fn send_offer(&self, mime_type: &str) {
        ZwlrDataControlOfferV1::send_offer(self, mime_type)
    }

    fn cancel(&self) {
        match self.location {
            IpcLocation::Clipboard => cancel_offer::<WlrClipboardIpc>(self),
            IpcLocation::PrimarySelection => cancel_offer::<WlrPrimarySelectionIpc>(self),
        }
    }

    fn get_seat(&self) -> Rc<WlSeatGlobal> {
        self.device.seat.clone()
    }

    fn is_privileged(&self) -> bool {
        true
    }
}

impl ZwlrDataControlOfferV1 {
    pub fn send_offer(&self, mime_type: &str) {
        self.client.event(Offer {
            self_id: self.id,
            mime_type,
        })
    }
}

impl ZwlrDataControlOfferV1RequestHandler for ZwlrDataControlOfferV1 {
    type Error = ZwlrDataControlOfferV1Error;

    fn receive(&self, req: Receive, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        match self.location {
            IpcLocation::Clipboard => {
                receive_data_offer::<WlrClipboardIpc>(self, req.mime_type, req.fd)
            }
            IpcLocation::PrimarySelection => {
                receive_data_offer::<WlrPrimarySelectionIpc>(self, req.mime_type, req.fd)
            }
        }
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        match self.location {
            IpcLocation::Clipboard => destroy_data_offer::<WlrClipboardIpc>(self),
            IpcLocation::PrimarySelection => destroy_data_offer::<WlrPrimarySelectionIpc>(self),
        }
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrDataControlOfferV1;
    version = self.device.version;
}

impl Object for ZwlrDataControlOfferV1 {
    fn break_loops(&self) {
        match self.location {
            IpcLocation::Clipboard => break_offer_loops::<WlrClipboardIpc>(self),
            IpcLocation::PrimarySelection => break_offer_loops::<WlrPrimarySelectionIpc>(self),
        }
    }
}

simple_add_obj!(ZwlrDataControlOfferV1);

#[derive(Debug, Error)]
pub enum ZwlrDataControlOfferV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrDataControlOfferV1Error, ClientError);
