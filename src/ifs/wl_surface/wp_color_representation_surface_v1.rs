use {
    crate::{
        client::{Client, ClientError},
        gfx_api::AlphaMode,
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WpColorRepresentationSurfaceV1Id,
            wp_color_representation_surface_v1::{
                Destroy, SetAlphaMode, SetChromaLocation, SetCoefficientsAndRange,
                WpColorRepresentationSurfaceV1RequestHandler,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpColorRepresentationSurfaceV1 {
    pub id: WpColorRepresentationSurfaceV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub surface: Rc<WlSurface>,
    pub supports_alpha_modes: bool,
}

pub const AM_PREMULTIPLIED_ELECTRICAL: u32 = 0;
pub const AM_PREMULTIPLIED_OPTICAL: u32 = 1;
pub const AM_STRAIGHT: u32 = 2;

impl WpColorRepresentationSurfaceV1 {
    pub fn install(self: &Rc<Self>) -> Result<(), WpColorRepresentationSurfaceV1Error> {
        if self.surface.color_representation_surface.is_some() {
            return Err(WpColorRepresentationSurfaceV1Error::HasSurface);
        }
        self.surface
            .color_representation_surface
            .set(Some(self.clone()));
        Ok(())
    }
}

impl WpColorRepresentationSurfaceV1RequestHandler for WpColorRepresentationSurfaceV1 {
    type Error = WpColorRepresentationSurfaceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.color_representation_surface.take();
        self.surface.pending.borrow_mut().alpha_mode = Some(Default::default());
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_alpha_mode(&self, req: SetAlphaMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let sam = self.supports_alpha_modes;
        let alpha_mode = match req.alpha_mode {
            AM_PREMULTIPLIED_ELECTRICAL => AlphaMode::PremultipliedElectrical,
            AM_PREMULTIPLIED_OPTICAL if sam => AlphaMode::PremultipliedOptical,
            AM_STRAIGHT if sam => AlphaMode::Straight,
            n => {
                return Err(WpColorRepresentationSurfaceV1Error::UnsupportedAlphaMode(n));
            }
        };
        self.surface.pending.borrow_mut().alpha_mode = Some(alpha_mode);
        Ok(())
    }

    fn set_coefficients_and_range(
        &self,
        req: SetCoefficientsAndRange,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Err(
            WpColorRepresentationSurfaceV1Error::UnsupportedCoefficientsAndRange(
                req.coefficients,
                req.range,
            ),
        )
    }

    fn set_chroma_location(
        &self,
        _req: SetChromaLocation,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

object_base! {
    self = WpColorRepresentationSurfaceV1;
    version = self.version;
}

impl Object for WpColorRepresentationSurfaceV1 {}

simple_add_obj!(WpColorRepresentationSurfaceV1);

#[derive(Debug, Error)]
pub enum WpColorRepresentationSurfaceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("wl_surface already has a color-representation extension")]
    HasSurface,
    #[error("{0} is not a supported alpha mode")]
    UnsupportedAlphaMode(u32),
    #[error("{0}/{1} are not supported coefficients and range")]
    UnsupportedCoefficientsAndRange(u32, u32),
}
efrom!(WpColorRepresentationSurfaceV1Error, ClientError);
