use {
    crate::{
        client::{Client, ClientError},
        cmm::cmm_description::ColorDescription,
        leaks::Tracker,
        object::{Object, Version},
        wire::{WpImageDescriptionReferenceV1Id, wp_image_description_reference_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpImageDescriptionReferenceV1 {
    pub id: WpImageDescriptionReferenceV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub description: Rc<ColorDescription>,
}

impl WpImageDescriptionReferenceV1RequestHandler for WpImageDescriptionReferenceV1 {
    type Error = WpImageDescriptionReferenceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpImageDescriptionReferenceV1;
    version = Version(1);
}

impl Object for WpImageDescriptionReferenceV1 {}

dedicated_add_obj!(
    WpImageDescriptionReferenceV1,
    WpImageDescriptionReferenceV1Id,
    wp_image_description_reference
);

#[derive(Debug, Error)]
pub enum WpImageDescriptionReferenceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpImageDescriptionReferenceV1Error, ClientError);
