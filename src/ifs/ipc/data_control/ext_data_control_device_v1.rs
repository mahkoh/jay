use {
    crate::{
        client::Client,
        ifs::{
            ipc::data_control::{
                ext_data_control_offer_v1::ExtDataControlOfferV1,
                ext_data_control_source_v1::ExtDataControlSourceV1,
                private::{
                    logic::{self, DataControlError},
                    DataControlDevice, DataControlDeviceData, DataControlIpc, DataControlOfferData,
                },
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            ext_data_control_device_v1::*, ExtDataControlDeviceV1Id, ExtDataControlOfferV1Id,
            ExtDataControlSourceV1Id,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtDataControlDeviceV1 {
    pub id: ExtDataControlDeviceV1Id,
    pub data: DataControlDeviceData<ExtDataControlIpc>,
    pub tracker: Tracker<Self>,
}

impl ExtDataControlDeviceV1 {
    pub fn new(
        id: ExtDataControlDeviceV1Id,
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

    pub fn send_data_offer(&self, offer: &Rc<ExtDataControlOfferV1>) {
        self.data.client.event(DataOffer {
            self_id: self.id,
            id: offer.id,
        })
    }

    pub fn send_selection(&self, offer: Option<&Rc<ExtDataControlOfferV1>>) {
        let id = offer.map(|o| o.id).unwrap_or(ExtDataControlOfferV1Id::NONE);
        self.data.client.event(Selection {
            self_id: self.id,
            id,
        })
    }

    pub fn send_primary_selection(&self, offer: Option<&Rc<ExtDataControlOfferV1>>) {
        let id = offer.map(|o| o.id).unwrap_or(ExtDataControlOfferV1Id::NONE);
        self.data.client.event(PrimarySelection {
            self_id: self.id,
            id,
        })
    }
}

impl ExtDataControlDeviceV1RequestHandler for ExtDataControlDeviceV1 {
    type Error = ExtDataControlDeviceV1Error;

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

pub struct ExtDataControlIpc;

impl DataControlIpc for ExtDataControlIpc {
    const PRIMARY_SELECTION_SINCE: Version = Version(1);
    type Device = ExtDataControlDeviceV1;
    type OfferId = ExtDataControlOfferV1Id;
    type Offer = ExtDataControlOfferV1;
    type SourceId = ExtDataControlSourceV1Id;
    type Source = ExtDataControlSourceV1;

    fn create_offer(id: Self::OfferId, data: DataControlOfferData<Self>) -> Rc<Self::Offer> {
        let rc = Rc::new(ExtDataControlOfferV1 {
            id,
            data,
            tracker: Default::default(),
        });
        track!(rc.data.client, rc);
        rc
    }
}

impl DataControlDevice for ExtDataControlDeviceV1 {
    type Ipc = ExtDataControlIpc;

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
    self = ExtDataControlDeviceV1;
    version = self.data.version;
}

impl Object for ExtDataControlDeviceV1 {
    fn break_loops(&self) {
        logic::data_device_break_loops(self);
    }
}

simple_add_obj!(ExtDataControlDeviceV1);

#[derive(Debug, Error)]
pub enum ExtDataControlDeviceV1Error {
    #[error(transparent)]
    DataControlError(#[from] DataControlError),
}
