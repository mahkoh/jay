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
            EiScrollId,
            ei_scroll::{
                ClientScroll, ClientScrollDiscrete, ClientScrollStop, EiScrollRequestHandler,
                Release, ServerScroll, ServerScrollDiscrete, ServerScrollStop,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct EiScroll {
    pub id: EiScrollId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
    pub device: Rc<EiDevice>,
}

ei_device_interface!(EiScroll, ei_scroll, scroll);

impl EiScroll {
    pub fn send_scroll(&self, x: Fixed, y: Fixed) {
        self.client.event(ServerScroll {
            self_id: self.id,
            x: x.to_f32(),
            y: y.to_f32(),
        });
    }

    pub fn send_scroll_discrete(&self, x: i32, y: i32) {
        self.client.event(ServerScrollDiscrete {
            self_id: self.id,
            x,
            y,
        });
    }

    pub fn send_scroll_stop(&self, x: bool, y: bool) {
        self.client.event(ServerScrollStop {
            self_id: self.id,
            x: x as _,
            y: y as _,
            is_cancel: 0,
        });
    }
}

impl EiScrollRequestHandler for EiScroll {
    type Error = EiScrollError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy()?;
        Ok(())
    }

    fn client_scroll(&self, req: ClientScroll, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.device.scroll_px[0].set(Some(req.x));
        self.device.scroll_px[1].set(Some(req.y));
        Ok(())
    }

    fn client_scroll_discrete(
        &self,
        req: ClientScrollDiscrete,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.device.scroll_v120[0].set(Some(req.x));
        self.device.scroll_v120[1].set(Some(req.y));
        Ok(())
    }

    fn client_scroll_stop(
        &self,
        req: ClientScrollStop,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.device.scroll_stop[0].set(Some(req.x != 0));
        self.device.scroll_stop[1].set(Some(req.y != 0));
        Ok(())
    }
}

ei_object_base! {
    self = EiScroll;
    version = self.version;
}

impl EiObject for EiScroll {}

#[derive(Debug, Error)]
pub enum EiScrollError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
}
efrom!(EiScrollError, EiClientError);
