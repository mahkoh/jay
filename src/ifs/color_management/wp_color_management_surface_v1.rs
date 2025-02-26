use {
    crate::{
        client::{Client, ClientError},
        ifs::color_management::consts::RENDER_INTENT_PERCEPTUAL,
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WpColorManagementSurfaceV1Id,
            wp_color_management_surface_v1::{
                Destroy, SetImageDescription, UnsetImageDescription,
                WpColorManagementSurfaceV1RequestHandler,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpColorManagementSurfaceV1 {
    pub id: WpColorManagementSurfaceV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl WpColorManagementSurfaceV1RequestHandler for WpColorManagementSurfaceV1 {
    type Error = WpColorManagementSurfaceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_image_description(
        &self,
        req: SetImageDescription,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let _ = self.client.lookup(req.image_description)?;
        if req.render_intent != RENDER_INTENT_PERCEPTUAL {
            return Err(WpColorManagementSurfaceV1Error::UnsupportedRenderIntent(
                req.render_intent,
            ));
        }
        Ok(())
    }

    fn unset_image_description(
        &self,
        _req: UnsetImageDescription,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

object_base! {
    self = WpColorManagementSurfaceV1;
    version = self.version;
}

impl Object for WpColorManagementSurfaceV1 {}

simple_add_obj!(WpColorManagementSurfaceV1);

#[derive(Debug, Error)]
pub enum WpColorManagementSurfaceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("{} is not a supported render intent", .0)]
    UnsupportedRenderIntent(u32),
}
efrom!(WpColorManagementSurfaceV1Error, ClientError);
