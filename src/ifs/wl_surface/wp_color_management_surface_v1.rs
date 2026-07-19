use crate::client::Client;
use crate::client::ClientError;
use crate::cmm::cmm_render_intent::RenderIntent;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::WpColorManagementSurfaceV1Id;
use crate::wire::wp_color_management_surface_v1::Destroy;
use crate::wire::wp_color_management_surface_v1::SetImageDescription;
use crate::wire::wp_color_management_surface_v1::UnsetImageDescription;
use crate::wire::wp_color_management_surface_v1::WpColorManagementSurfaceV1RequestHandler;
use std::rc::Rc;
use thiserror::Error;

pub struct WpColorManagementSurfaceV1 {
    pub id: WpColorManagementSurfaceV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub surface: Rc<WlSurface>,
}

impl WpColorManagementSurfaceV1 {
    pub fn install(self: &Rc<Self>) -> Result<(), WpColorManagementSurfaceV1Error> {
        if self.surface.color_management_surface.is_some() {
            return Err(WpColorManagementSurfaceV1Error::HasSurface);
        }
        self.surface
            .color_management_surface
            .set(Some(self.clone()));
        Ok(())
    }
}

impl WpColorManagementSurfaceV1RequestHandler for WpColorManagementSurfaceV1 {
    type Error = WpColorManagementSurfaceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.color_management_surface.take();
        self.surface.pending.borrow_mut().color_description = Some(None);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_image_description(
        &self,
        req: SetImageDescription,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let Some(intent) = RenderIntent::from_wayland(req.render_intent, self.version) else {
            return Err(WpColorManagementSurfaceV1Error::UnsupportedRenderIntent(
                req.render_intent,
            ));
        };
        let desc = self.client.lookup(req.image_description)?;
        let Some(desc) = &desc.description else {
            return Err(WpColorManagementSurfaceV1Error::NotReady);
        };
        self.surface.pending.borrow_mut().color_description = Some(Some((intent, desc.clone())));
        Ok(())
    }

    fn unset_image_description(
        &self,
        _req: UnsetImageDescription,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.surface.pending.borrow_mut().color_description = Some(None);
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
    #[error("wl_surface already has a color-management extension")]
    HasSurface,
    #[error("The color description is not ready")]
    NotReady,
}
efrom!(WpColorManagementSurfaceV1Error, ClientError);
