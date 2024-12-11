use {
    crate::{
        ei::{
            ei_client::{EiClient, EiClientError},
            ei_ifs::ei_device::{EiDevice, EiDeviceInterface},
            ei_object::{EiObject, EiVersion},
        },
        fixed::Fixed,
        leaks::Tracker,
        utils::clonecell::UnsafeCellCloneSafe,
        wire_ei::{
            ei_touchscreen::{
                ClientCancel, ClientDown, ClientMotion, ClientUp, EiTouchscreenRequestHandler,
                Release, ServerCancel, ServerDown, ServerMotion, ServerUp,
            },
            EiTouchscreenId,
        },
    },
    std::{collections::hash_map::Entry, rc::Rc},
    thiserror::Error,
};

pub struct EiTouchscreen {
    pub id: EiTouchscreenId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
    pub device: Rc<EiDevice>,
}

#[derive(Copy, Clone, Debug)]
pub enum TouchChange {
    Down(f32, f32),
    Motion(f32, f32),
    Up,
    Cancel,
}

unsafe impl UnsafeCellCloneSafe for TouchChange {}

ei_device_interface!(EiTouchscreen, ei_touchscreen, touchscreen);

impl EiTouchscreen {
    pub fn send_down(&self, touchid: u32, x: Fixed, y: Fixed) {
        self.client.event(ServerDown {
            self_id: self.id,
            touchid,
            x: x.to_f32(),
            y: y.to_f32(),
        });
    }

    pub fn send_motion(&self, touchid: u32, x: Fixed, y: Fixed) {
        self.client.event(ServerMotion {
            self_id: self.id,
            touchid,
            x: x.to_f32(),
            y: y.to_f32(),
        });
    }

    pub fn send_up(&self, touchid: u32) {
        self.client.event(ServerUp {
            self_id: self.id,
            touchid,
        });
    }

    pub fn send_cancel(&self, touchid: u32) {
        self.client.event(ServerCancel {
            self_id: self.id,
            touchid,
        });
    }

    fn set_client_event(&self, touchid: u32, event: TouchChange) -> Result<(), EiTouchscreenError> {
        match self.device.touch_changes.lock().entry(touchid) {
            Entry::Occupied(mut o) => {
                use TouchChange::*;
                match (o.get(), event) {
                    (Motion(_, _), Motion(_, _))
                    | (Down(_, _), Down(_, _))
                    | (Up, Up)
                    | (Cancel, Cancel) => {
                        o.insert(event);
                        Ok(())
                    }
                    _ => Err(EiTouchscreenError::InvalidEventCombination),
                }
            }
            Entry::Vacant(v) => {
                v.insert(event);
                Ok(())
            }
        }
    }
}

impl EiTouchscreenRequestHandler for EiTouchscreen {
    type Error = EiTouchscreenError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy()?;
        Ok(())
    }

    fn client_down(&self, req: ClientDown, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.set_client_event(req.touchid, TouchChange::Down(req.x, req.y))
    }

    fn client_motion(&self, req: ClientMotion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.set_client_event(req.touchid, TouchChange::Motion(req.x, req.y))
    }

    fn client_up(&self, req: ClientUp, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.set_client_event(req.touchid, TouchChange::Up)
    }

    fn client_cancel(&self, req: ClientCancel, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.set_client_event(req.touchid, TouchChange::Cancel)
    }
}

ei_object_base! {
    self = EiTouchscreen;
    version = self.version;
}

impl EiObject for EiTouchscreen {}

#[derive(Debug, Error)]
pub enum EiTouchscreenError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
    #[error("Touch frame contains an invalid combination of events")]
    InvalidEventCombination,
}
efrom!(EiTouchscreenError, EiClientError);
