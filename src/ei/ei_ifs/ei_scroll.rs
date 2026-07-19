use crate::ei::ei_client::EiClient;
use crate::ei::ei_client::EiClientError;
use crate::ei::ei_ifs::ei_device::EiDevice;
use crate::ei::ei_ifs::ei_device::EiDeviceInterface;
use crate::ei::ei_object::EiObject;
use crate::ei::ei_object::EiVersion;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::wl_pointer::HORIZONTAL_SCROLL;
use crate::ifs::wl_seat::wl_pointer::VERTICAL_SCROLL;
use crate::leaks::Tracker;
use crate::wire_ei::EiScrollId;
use crate::wire_ei::ei_scroll::ClientScroll;
use crate::wire_ei::ei_scroll::ClientScrollDiscrete;
use crate::wire_ei::ei_scroll::ClientScrollStop;
use crate::wire_ei::ei_scroll::EiScrollRequestHandler;
use crate::wire_ei::ei_scroll::Release;
use crate::wire_ei::ei_scroll::ServerScroll;
use crate::wire_ei::ei_scroll::ServerScrollDiscrete;
use crate::wire_ei::ei_scroll::ServerScrollStop;
use std::rc::Rc;
use thiserror::Error;

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
        self.device.scroll_px[HORIZONTAL_SCROLL].set(Some(req.x));
        self.device.scroll_px[VERTICAL_SCROLL].set(Some(req.y));
        Ok(())
    }

    fn client_scroll_discrete(
        &self,
        req: ClientScrollDiscrete,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.device.scroll_v120[HORIZONTAL_SCROLL].set(Some(req.x));
        self.device.scroll_v120[VERTICAL_SCROLL].set(Some(req.y));
        Ok(())
    }

    fn client_scroll_stop(
        &self,
        req: ClientScrollStop,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.device.scroll_stop[HORIZONTAL_SCROLL].set(Some(req.x != 0));
        self.device.scroll_stop[VERTICAL_SCROLL].set(Some(req.y != 0));
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
