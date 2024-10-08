use {
    crate::{
        client::Client,
        ifs::{
            ipc::data_control::{
                private::{
                    logic::{self, DataControlError},
                    DataControlDevice, DataControlDeviceData, DataControlIpc, DataControlOfferData,
                },
                zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
                zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            zwlr_data_control_device_v1::*, ZwlrDataControlDeviceV1Id, ZwlrDataControlOfferV1Id,
            ZwlrDataControlSourceV1Id,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub const PRIMARY_SELECTION_SINCE: Version = Version(2);

pub struct ZwlrDataControlDeviceV1 {
    pub id: ZwlrDataControlDeviceV1Id,
    pub data: DataControlDeviceData<WlrDataControlIpc>,
    pub tracker: Tracker<Self>,
}

impl ZwlrDataControlDeviceV1 {
    pub fn new(
        id: ZwlrDataControlDeviceV1Id,
        client: &Rc<Client>,
        version: Version,
        seat: &Rc<WlSeatGlobal>,
    ) -> Self {
        Self {
            id,
            data: DataControlDeviceData {
                data_control_device_id: client.state.data_control_device_ids.next(),
                client: client.clone(),
                version,
                seat: seat.clone(),
                clipboard_data: Default::default(),
                primary_selection_data: Default::default(),
            },
            tracker: Default::default(),
        }
    }

    pub fn send_data_offer(&self, offer: &Rc<ZwlrDataControlOfferV1>) {
        self.data.client.event(DataOffer {
            self_id: self.id,
            id: offer.id,
        })
    }

    pub fn send_selection(&self, offer: Option<&Rc<ZwlrDataControlOfferV1>>) {
        let id = offer
            .map(|o| o.id)
            .unwrap_or(ZwlrDataControlOfferV1Id::NONE);
        self.data.client.event(Selection {
            self_id: self.id,
            id,
        })
    }

    pub fn send_primary_selection(&self, offer: Option<&Rc<ZwlrDataControlOfferV1>>) {
        let id = offer
            .map(|o| o.id)
            .unwrap_or(ZwlrDataControlOfferV1Id::NONE);
        self.data.client.event(PrimarySelection {
            self_id: self.id,
            id,
        })
    }
}

impl ZwlrDataControlDeviceV1RequestHandler for ZwlrDataControlDeviceV1 {
    type Error = ZwlrDataControlDeviceV1Error;

    fn set_selection(&self, req: SetSelection, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        logic::device_set_selection(self, req.source.is_some().then_some(req.source))?;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        logic::device_destroy(self)?;
        Ok(())
    }

    fn set_primary_selection(
        &self,
        req: SetPrimarySelection,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        logic::device_set_primary_selection(self, req.source.is_some().then_some(req.source))?;
        Ok(())
    }
}

pub struct WlrDataControlIpc;

impl DataControlIpc for WlrDataControlIpc {
    const PRIMARY_SELECTION_SINCE: Version = PRIMARY_SELECTION_SINCE;
    type Device = ZwlrDataControlDeviceV1;
    type OfferId = ZwlrDataControlOfferV1Id;
    type Offer = ZwlrDataControlOfferV1;
    type SourceId = ZwlrDataControlSourceV1Id;
    type Source = ZwlrDataControlSourceV1;

    fn create_offer(id: Self::OfferId, data: DataControlOfferData<Self>) -> Rc<Self::Offer> {
        let rc = Rc::new(ZwlrDataControlOfferV1 {
            id,
            data,
            tracker: Default::default(),
        });
        track!(rc.data.client, rc);
        rc
    }
}

impl DataControlDevice for ZwlrDataControlDeviceV1 {
    type Ipc = WlrDataControlIpc;

    fn data(&self) -> &DataControlDeviceData<Self::Ipc> {
        &self.data
    }

    fn send_data_offer(&self, offer: &Rc<<Self::Ipc as DataControlIpc>::Offer>) {
        self.send_data_offer(offer)
    }

    fn send_selection(&self, offer: Option<&Rc<<Self::Ipc as DataControlIpc>::Offer>>) {
        self.send_selection(offer)
    }

    fn send_primary_selection(&self, offer: Option<&Rc<<Self::Ipc as DataControlIpc>::Offer>>) {
        self.send_primary_selection(offer)
    }
}

object_base! {
    self = ZwlrDataControlDeviceV1;
    version = self.data.version;
}

impl Object for ZwlrDataControlDeviceV1 {
    fn break_loops(&self) {
        logic::data_device_break_loops(self);
    }
}

simple_add_obj!(ZwlrDataControlDeviceV1);

#[derive(Debug, Error)]
pub enum ZwlrDataControlDeviceV1Error {
    #[error(transparent)]
    DataControlError(#[from] DataControlError),
}
