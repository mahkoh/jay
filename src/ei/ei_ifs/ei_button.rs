use {
    crate::{
        backend::ButtonState,
        ei::{
            ei_client::{EiClient, EiClientError},
            ei_ifs::ei_device::{EiDevice, EiDeviceInterface},
            ei_object::{EiObject, EiVersion},
        },
        leaks::Tracker,
        wire_ei::{
            EiButtonId,
            ei_button::{ClientButton, EiButtonRequestHandler, Release, ServerButton},
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

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
