use crate::client::Client;
use crate::cmm::cmm_description::ColorDescription;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WpImageDescriptionReferenceV1Id;
use crate::wire::wp_image_description_reference_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
pub struct WpImageDescriptionReferenceV1 {
    pub id: WpImageDescriptionReferenceV1Id,
    pub version: Version,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub description: Rc<ColorDescription>,
}

impl WpImageDescriptionReferenceV1RequestHandler for WpImageDescriptionReferenceV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
