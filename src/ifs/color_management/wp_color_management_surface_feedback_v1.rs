use {
    crate::{
        client::{Client, ClientError},
        ifs::color_management::wp_image_description_v1::WpImageDescriptionV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WpColorManagementSurfaceFeedbackV1Id, WpImageDescriptionV1Id,
            wp_color_management_surface_feedback_v1::*,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpColorManagementSurfaceFeedbackV1 {
    pub id: WpColorManagementSurfaceFeedbackV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl WpColorManagementSurfaceFeedbackV1 {
    fn get_description(
        &self,
        id: WpImageDescriptionV1Id,
    ) -> Result<(), WpColorManagementSurfaceFeedbackV1Error> {
        let obj = Rc::new(WpImageDescriptionV1 {
            id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            description: self.client.state.color_manager.srgb_srgb().clone(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_ready();
        Ok(())
    }
}

impl WpColorManagementSurfaceFeedbackV1RequestHandler for WpColorManagementSurfaceFeedbackV1 {
    type Error = WpColorManagementSurfaceFeedbackV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_preferred(&self, req: GetPreferred, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.get_description(req.image_description)
    }

    fn get_preferred_parametric(
        &self,
        req: GetPreferredParametric,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.get_description(req.image_description)
    }
}

object_base! {
    self = WpColorManagementSurfaceFeedbackV1;
    version = self.version;
}

impl Object for WpColorManagementSurfaceFeedbackV1 {}

simple_add_obj!(WpColorManagementSurfaceFeedbackV1);

#[derive(Debug, Error)]
pub enum WpColorManagementSurfaceFeedbackV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpColorManagementSurfaceFeedbackV1Error, ClientError);
