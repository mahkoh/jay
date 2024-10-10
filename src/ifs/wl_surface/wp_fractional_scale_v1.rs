use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        scale::Scale,
        utils::cell_ext::CellExt,
        wire::{wp_fractional_scale_v1::*, WpFractionalScaleV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpFractionalScaleV1 {
    pub id: WpFractionalScaleV1Id,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpFractionalScaleV1 {
    pub fn new(id: WpFractionalScaleV1Id, surface: &Rc<WlSurface>, version: Version) -> Self {
        Self {
            id,
            client: surface.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
            version,
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpFractionalScaleError> {
        if self.surface.fractional_scale.is_some() {
            return Err(WpFractionalScaleError::Exists);
        }
        self.surface.fractional_scale.set(Some(self.clone()));
        Ok(())
    }

    pub fn send_preferred_scale(&self) {
        let scale = match self.client.wire_scale.is_some() {
            true => Scale::from_int(1),
            false => self.surface.output.get().global.persistent.scale.get(),
        };
        self.client.event(PreferredScale {
            self_id: self.id,
            scale: scale.to_wl(),
        });
    }
}

impl WpFractionalScaleV1RequestHandler for WpFractionalScaleV1 {
    type Error = WpFractionalScaleError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.fractional_scale.take();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpFractionalScaleV1;
    version = self.version;
}

impl Object for WpFractionalScaleV1 {}

simple_add_obj!(WpFractionalScaleV1);

#[derive(Debug, Error)]
pub enum WpFractionalScaleError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a fractional scale extension attached")]
    Exists,
}
efrom!(WpFractionalScaleError, ClientError);
