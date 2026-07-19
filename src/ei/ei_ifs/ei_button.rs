use crate::backend::ButtonState;
use crate::ei::ei_client::EiClient;
use crate::ei::ei_client::EiClientError;
use crate::ei::ei_ifs::ei_device::EiDevice;
use crate::ei::ei_ifs::ei_device::EiDeviceInterface;
use crate::ei::ei_object::EiObject;
use crate::ei::ei_object::EiVersion;
use crate::leaks::Tracker;
use crate::wire_ei::EiButtonId;
use crate::wire_ei::ei_button::ClientButton;
use crate::wire_ei::ei_button::EiButtonRequestHandler;
use crate::wire_ei::ei_button::Release;
use crate::wire_ei::ei_button::ServerButton;
use std::rc::Rc;
use thiserror::Error;

pub struct EiButton {
    pub id: EiButtonId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
    pub device: Rc<EiDevice>,
}

ei_device_interface!(EiButton, ei_button, button);

impl EiButton {
    pub fn send_button(&self, button: u32, state: ButtonState) {
        self.client.event(ServerButton {
            self_id: self.id,
            button,
            state: state as _,
        });
    }
}

impl EiButtonRequestHandler for EiButton {
    type Error = EiButtonError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy()?;
        Ok(())
    }

    fn client_button(&self, req: ClientButton, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pressed = match req.state {
            0 => ButtonState::Released,
            1 => ButtonState::Pressed,
            _ => return Err(EiButtonError::InvalidButtonState(req.state)),
        };
        self.device.button_changes.push((req.button, pressed));
        Ok(())
    }
}

ei_object_base! {
    self = EiButton;
    version = self.version;
}

impl EiObject for EiButton {}

#[derive(Debug, Error)]
pub enum EiButtonError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
    #[error("Invalid button state {0}")]
    InvalidButtonState(u32),
}
efrom!(EiButtonError, EiClientError);
