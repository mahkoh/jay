use {
    crate::{
        client::ClientId,
        ifs::{
            ipc::{
                DataOffer, DataOfferId, DynDataOffer, IpcLocation, OfferData, cancel_offer,
                x_data_device::{XClipboardIpc, XIpcDevice, XPrimarySelectionIpc},
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        xwayland::XWaylandEvent,
    },
    XWaylandEvent::IpcAddOfferMimeType,
    std::rc::Rc,
};

pub struct XDataOffer {
    pub offer_id: DataOfferId,
    pub device: Rc<XIpcDevice>,
    pub data: OfferData<XIpcDevice>,
    pub tracker: Tracker<Self>,
    pub location: IpcLocation,
}

impl DataOffer for XDataOffer {
    type Device = XIpcDevice;

    fn offer_data(&self) -> &OfferData<Self::Device> {
        &self.data
    }
}

impl DynDataOffer for XDataOffer {
    fn offer_id(&self) -> DataOfferId {
        self.offer_id
    }

    fn client_id(&self) -> ClientId {
        self.device.client.id
    }

    fn send_offer(&self, mime_type: &str) {
        self.device.state.xwayland.queue.push(IpcAddOfferMimeType {
            location: self.location,
            seat: self.device.seat.id(),
            offer: self.offer_id,
            mime_type: mime_type.to_string(),
        })
    }

    fn cancel(&self) {
        match self.location {
            IpcLocation::Clipboard => cancel_offer::<XClipboardIpc>(self),
            IpcLocation::PrimarySelection => cancel_offer::<XPrimarySelectionIpc>(self),
        }
    }

    fn get_seat(&self) -> Rc<WlSeatGlobal> {
        self.device.seat.clone()
    }
}
