use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_alpha_modifier_surface_v1::*, WpAlphaModifierSurfaceV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpAlphaModifierSurfaceV1 {
    pub id: WpAlphaModifierSurfaceV1Id,
    pub version: Version,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
}

impl WpAlphaModifierSurfaceV1 {
    pub fn new(id: WpAlphaModifierSurfaceV1Id, surface: &Rc<WlSurface>, version: Version) -> Self {
        Self {
            id,
            version,
            client: surface.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpAlphaModifierSurfaceV1Error> {
        if self.surface.alpha_modifier.is_some() {
            return Err(WpAlphaModifierSurfaceV1Error::Exists);
        }
        self.surface.alpha_modifier.set(Some(self.clone()));
        Ok(())
    }
}

impl WpAlphaModifierSurfaceV1RequestHandler for WpAlphaModifierSurfaceV1 {
    type Error = WpAlphaModifierSurfaceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.alpha_modifier.take();
        self.surface.pending.borrow_mut().alpha_multiplier = Some(None);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_multiplier(&self, req: SetMultiplier, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let multiplier = if req.factor == u32::MAX {
            None
        } else {
            Some(((req.factor as f64) / (u32::MAX as f64)) as f32)
        };
        self.surface.pending.borrow_mut().alpha_multiplier = Some(multiplier);
        Ok(())
    }
}

object_base! {
    self = WpAlphaModifierSurfaceV1;
    version = self.version;
}

impl Object for WpAlphaModifierSurfaceV1 {}

simple_add_obj!(WpAlphaModifierSurfaceV1);

#[derive(Debug, Error)]
pub enum WpAlphaModifierSurfaceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has an alpha modifier extension attached")]
    Exists,
}
efrom!(WpAlphaModifierSurfaceV1Error, ClientError);
