use {
    crate::{
        client::ClientError,
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
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
    pub version: Version,
}

impl WpTearingControlV1 {
    pub fn install(self: &Rc<Self>) -> Result<(), WpTearingControlV1Error> {
        if self.surface.tearing_control.is_some() {
            return Err(WpTearingControlV1Error::AlreadyAttached(self.surface.id));
        }
        self.surface.tearing_control.set(Some(self.clone()));
        Ok(())
    }
}

impl WpTearingControlV1RequestHandler for WpTearingControlV1 {
    type Error = WpTearingControlV1Error;

    fn set_presentation_hint(
        &self,
        req: SetPresentationHint,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let tearing = match req.hint {
            VSYNC => false,
            ASYNC => true,
            _ => return Err(WpTearingControlV1Error::UnknownPresentationHint(req.hint)),
        };
        self.surface.pending.borrow_mut().tearing = Some(tearing);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.pending.borrow_mut().tearing = Some(false);
        self.surface.tearing_control.take();
        self.surface.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpTearingControlV1;
    version = self.version;
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
}
efrom!(WpTearingControlV1Error, ClientError);
