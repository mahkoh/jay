use {
    crate::{
        client::ClientError,
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_tearing_control_v1::*, WlSurfaceId, WpTearingControlV1Id},
    },
    std::{fmt::Debug, rc::Rc},
    thiserror::Error,
};

const VSYNC: u32 = 0;
const ASYNC: u32 = 1;

pub struct WpTearingControlV1 {
    pub id: WpTearingControlV1Id,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
}

impl WpTearingControlV1 {
    pub fn install(self: &Rc<Self>) -> Result<(), WpTearingControlV1Error> {
        if self.surface.tearing_control.get().is_some() {
            return Err(WpTearingControlV1Error::AlreadyAttached(self.surface.id));
        }
        self.surface.tearing_control.set(Some(self.clone()));
        Ok(())
    }

    fn set_presentation_hint(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WpTearingControlV1Error> {
        let req: SetPresentationHint = self.surface.client.parse(self, parser)?;
        let tearing = match req.hint {
            VSYNC => false,
            ASYNC => true,
            _ => return Err(WpTearingControlV1Error::UnknownPresentationHint(req.hint)),
        };
        self.surface.pending.tearing.set(Some(tearing));
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WpTearingControlV1Error> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
        self.surface.pending.tearing.set(Some(false));
        self.surface.tearing_control.take();
        self.surface.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpTearingControlV1;

    SET_PRESENTATION_HINT => set_presentation_hint,
    DESTROY => destroy,
}

impl Object for WpTearingControlV1 {}

simple_add_obj!(WpTearingControlV1);

#[derive(Debug, Error)]
pub enum WpTearingControlV1Error {
    #[error("Surface {0} already has a wp_tearing_control")]
    AlreadyAttached(WlSurfaceId),
    #[error("Unknown presentation hint {0}")]
    UnknownPresentationHint(u32),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(WpTearingControlV1Error, ClientError);
efrom!(WpTearingControlV1Error, MsgParserError);
