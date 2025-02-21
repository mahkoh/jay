use {
    crate::{
        ei::{
            ei_client::{EiClient, EiClientError},
            ei_ifs::ei_device::{EiDevice, EiDeviceInterface},
            ei_object::{EiObject, EiVersion},
        },
        fixed::Fixed,
        leaks::Tracker,
        wire_ei::{
            EiPointerAbsoluteId,
            ei_pointer_absolute::{
                ClientMotionAbsolute, EiPointerAbsoluteRequestHandler, Release,
                ServerMotionAbsolute,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct EiPointerAbsolute {
    pub id: EiPointerAbsoluteId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
    pub device: Rc<EiDevice>,
}

ei_device_interface!(EiPointerAbsolute, ei_pointer_absolute, pointer_absolute);

impl EiPointerAbsolute {
    pub fn send_motion_absolute(&self, x: Fixed, y: Fixed) {
        self.client.event(ServerMotionAbsolute {
            self_id: self.id,
            x: x.to_f32(),
            y: y.to_f32(),
        });
    }
}

impl EiPointerAbsoluteRequestHandler for EiPointerAbsolute {
    type Error = EiCallbackError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy()?;
        Ok(())
    }

    fn client_motion_absolute(
        &self,
        req: ClientMotionAbsolute,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.device.absolute_motion.set(Some((req.x, req.y)));
        Ok(())
    }
}

ei_object_base! {
    self = EiPointerAbsolute;
    version = self.version;
}

impl EiObject for EiPointerAbsolute {}

#[derive(Debug, Error)]
pub enum EiCallbackError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
}
efrom!(EiCallbackError, EiClientError);
