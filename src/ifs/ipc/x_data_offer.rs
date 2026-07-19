use crate::client::ClientId;
use crate::ifs::ipc::DataOffer;
use crate::ifs::ipc::DataOfferId;
use crate::ifs::ipc::DynDataOffer;
use crate::ifs::ipc::IpcLocation;
use crate::ifs::ipc::OfferData;
use crate::ifs::ipc::cancel_offer;
use crate::ifs::ipc::x_data_device::XClipboardIpc;
use crate::ifs::ipc::x_data_device::XIpcDevice;
use crate::ifs::ipc::x_data_device::XPrimarySelectionIpc;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::leaks::Tracker;
use crate::xwayland::XWaylandEvent;
use XWaylandEvent::IpcAddOfferMimeType;
use std::rc::Rc;

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
