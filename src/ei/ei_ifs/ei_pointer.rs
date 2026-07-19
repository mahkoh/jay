use crate::ei::ei_client::EiClient;
use crate::ei::ei_client::EiClientError;
use crate::ei::ei_ifs::ei_device::EiDevice;
use crate::ei::ei_ifs::ei_device::EiDeviceInterface;
use crate::ei::ei_object::EiObject;
use crate::ei::ei_object::EiVersion;
use crate::fixed::Fixed;
use crate::leaks::Tracker;
use crate::wire_ei::EiPointerId;
use crate::wire_ei::ei_pointer::ClientMotionRelative;
use crate::wire_ei::ei_pointer::EiPointerRequestHandler;
use crate::wire_ei::ei_pointer::Release;
use crate::wire_ei::ei_pointer::ServerMotionRelative;
use std::rc::Rc;
use thiserror::Error;

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
