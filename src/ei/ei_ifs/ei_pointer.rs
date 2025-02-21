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
            EiPointerId,
            ei_pointer::{
                ClientMotionRelative, EiPointerRequestHandler, Release, ServerMotionRelative,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct EiPointer {
    pub id: EiPointerId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
    pub device: Rc<EiDevice>,
}

ei_device_interface!(EiPointer, ei_pointer, pointer);

impl EiPointer {
    pub fn send_motion(&self, dx: Fixed, dy: Fixed) {
        self.client.event(ServerMotionRelative {
            self_id: self.id,
            x: dx.to_f32(),
            y: dy.to_f32(),
        });
    }
}

impl EiPointerRequestHandler for EiPointer {
    type Error = EiPointerError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy()?;
        Ok(())
    }

    fn client_motion_relative(
        &self,
        req: ClientMotionRelative,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.device.relative_motion.set(Some((req.x, req.y)));
        Ok(())
    }
}

ei_object_base! {
    self = EiPointer;
    version = self.version;
}

impl EiObject for EiPointer {}

#[derive(Debug, Error)]
pub enum EiPointerError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
}
efrom!(EiPointerError, EiClientError);
