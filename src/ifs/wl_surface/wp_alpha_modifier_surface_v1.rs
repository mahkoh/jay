use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_alpha_modifier_surface_v1::*, WpAlphaModifierSurfaceV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpAlphaModifierSurfaceV1 {
    pub id: WpAlphaModifierSurfaceV1Id,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
}

impl WpAlphaModifierSurfaceV1 {
    pub fn new(id: WpAlphaModifierSurfaceV1Id, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
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

    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), WpAlphaModifierSurfaceV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.surface.alpha_modifier.take();
        self.surface.pending.alpha_multiplier.set(Some(None));
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_multiplier(&self, msg: MsgParser<'_, '_>) -> Result<(), WpAlphaModifierSurfaceV1Error> {
        let req: SetMultiplier = self.client.parse(self, msg)?;
        let multiplier = if req.factor == u32::MAX {
            None
        } else {
            Some(((req.factor as f64) / (u32::MAX as f64)) as f32)
        };
        self.surface.pending.alpha_multiplier.set(Some(multiplier));
        Ok(())
    }
}

object_base! {
    self = WpAlphaModifierSurfaceV1;

    DESTROY => destroy,
    SET_MULTIPLIER => set_multiplier,
}

impl Object for WpAlphaModifierSurfaceV1 {}

simple_add_obj!(WpAlphaModifierSurfaceV1);

#[derive(Debug, Error)]
pub enum WpAlphaModifierSurfaceV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has an alpha modifier extension attached")]
    Exists,
}
efrom!(WpAlphaModifierSurfaceV1Error, MsgParserError);
efrom!(WpAlphaModifierSurfaceV1Error, ClientError);
