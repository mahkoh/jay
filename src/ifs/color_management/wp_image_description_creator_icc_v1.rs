use {
    crate::{
        client::Client,
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WpImageDescriptionCreatorIccV1Id,
            wp_image_description_creator_icc_v1::{
                Create, SetIccFile, WpImageDescriptionCreatorIccV1RequestHandler,
            },
        },
    },
    std::{convert::Infallible, rc::Rc},
};

#[expect(dead_code)]
pub struct WpImageDescriptionCreatorIccV1 {
    pub id: WpImageDescriptionCreatorIccV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl WpImageDescriptionCreatorIccV1RequestHandler for WpImageDescriptionCreatorIccV1 {
    type Error = Infallible;

    fn create(&self, _req: Create, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        unreachable!()
    }

    fn set_icc_file(&self, _req: SetIccFile, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        unreachable!()
    }
}

object_base! {
    self = WpImageDescriptionCreatorIccV1;
    version = self.version;
}

impl Object for WpImageDescriptionCreatorIccV1 {}

simple_add_obj!(WpImageDescriptionCreatorIccV1);
