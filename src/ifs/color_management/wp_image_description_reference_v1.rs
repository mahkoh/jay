use crate::client::Client;
use crate::client::ClientError;
use crate::cmm::cmm_description::ColorDescription;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WpImageDescriptionReferenceV1Id;
use crate::wire::wp_image_description_reference_v1::*;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[expect(unused)]
#[derive(Object)]
pub struct WpImageDescriptionReferenceV1 {
    id: WpImageDescriptionReferenceV1Id,
    version: Version,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    pub description: Rc<ColorDescription>,
}

impl WpImageDescriptionReferenceV1RequestHandler for WpImageDescriptionReferenceV1 {
    type Error = WpImageDescriptionReferenceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum WpImageDescriptionReferenceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpImageDescriptionReferenceV1Error, ClientError);
