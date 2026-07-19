use crate::client::Client;
use crate::client::ClientError;
use crate::cmm::cmm_description::ColorDescription;
use crate::ifs::color_management::UNIQUE_CM_IDS_SINCE;
use crate::ifs::color_management::wp_image_description_v1::WpImageDescriptionV1;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::tree::TreeTimeline::LiveTL;
use crate::wire::WpColorManagementSurfaceFeedbackV1Id;
use crate::wire::WpImageDescriptionV1Id;
use crate::wire::wp_color_management_surface_feedback_v1::*;
use std::rc::Rc;
use thiserror::Error;

pub struct WpColorManagementSurfaceFeedbackV1 {
    pub id: WpColorManagementSurfaceFeedbackV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub surface: Rc<WlSurface>,
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
            description: Some(
                self.surface.get_output().node_state[LiveTL]
                    .color_description
                    .get(),
            ),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_ready();
        Ok(())
    }

    pub fn send_preferred_changed(&self, cd: &ColorDescription) {
        let identity = cd.id.raw();
        if self.version >= UNIQUE_CM_IDS_SINCE {
            self.client.event(PreferredChanged2 {
                self_id: self.id,
                identity,
            });
        } else {
            self.client.event(PreferredChanged {
                self_id: self.id,
                identity: identity as u32,
            });
        }
    }
}

impl WpColorManagementSurfaceFeedbackV1RequestHandler for WpColorManagementSurfaceFeedbackV1 {
    type Error = WpColorManagementSurfaceFeedbackV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        self.surface.remove_color_management_feedback(self);
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
