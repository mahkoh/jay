use {
    crate::{
        ifs::ipc::data_control::{
            ext_data_control_device_v1::ExtDataControlIpc,
            private::{
                DataControlOffer, DataControlOfferData,
                logic::{self, DataControlError},
            },
        },
        leaks::Tracker,
        object::Object,
        wire::{ExtDataControlOfferV1Id, ext_data_control_offer_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtDataControlOfferV1 {
    pub id: ExtDataControlOfferV1Id,
    pub data: DataControlOfferData<ExtDataControlIpc>,
    pub tracker: Tracker<Self>,
}

impl DataControlOffer for ExtDataControlOfferV1 {
    type Ipc = ExtDataControlIpc;

    fn data(&self) -> &DataControlOfferData<Self::Ipc> {
        &self.data
    }

    fn send_offer(&self, mime_type: &str) {
        self.send_offer(mime_type);
    }
}

impl ExtDataControlOfferV1 {
    pub fn send_offer(&self, mime_type: &str) {
        self.data.client.event(Offer {
            self_id: self.id,
            mime_type,
        })
    }
}

impl ExtDataControlOfferV1RequestHandler for ExtDataControlOfferV1 {
    type Error = ExtDataControlOfferV1Error;

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
    self = ExtDataControlOfferV1;
    version = self.data.device.data.version;
}

impl Object for ExtDataControlOfferV1 {
    fn break_loops(&self) {
        logic::data_offer_break_loops(self);
    }
}

simple_add_obj!(ExtDataControlOfferV1);

#[derive(Debug, Error)]
pub enum ExtDataControlOfferV1Error {
    #[error(transparent)]
    DataControlError(#[from] DataControlError),
}
