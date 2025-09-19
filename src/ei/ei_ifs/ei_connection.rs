use {
    crate::{
        ei::{
            EiContext,
            ei_client::{EiClient, EiClientError},
            ei_ifs::{
                ei_callback::EiCallback,
                ei_pingpong::EiPingpong,
                ei_seat::{
                    EI_CAP_BUTTON, EI_CAP_KEYBOARD, EI_CAP_POINTER, EI_CAP_POINTER_ABSOLUTE,
                    EI_CAP_SCROLL, EI_CAP_TOUCHSCREEN, EiSeat,
                },
            },
            ei_object::{EiObject, EiObjectId, EiVersion},
        },
        ifs::wl_seat::WlSeatGlobal,
        leaks::Tracker,
        wire_ei::{
            EiButton, EiConnectionId, EiKeyboard, EiPointer, EiPointerAbsolute, EiScroll,
            EiTouchscreen,
            ei_connection::{
                Disconnect, Disconnected, EiConnectionRequestHandler, InvalidObject, Ping, Seat,
            },
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct EiConnection {
    pub id: EiConnectionId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
}

impl EiConnection {
    pub fn send_invalid_object(&self, id: EiObjectId) {
        self.client.event(InvalidObject {
            self_id: self.id,
            last_serial: self.client.last_serial(),
            invalid_id: id,
        });
    }

    pub fn send_disconnected(&self, error: Option<&str>) {
        self.client.event(Disconnected {
            self_id: self.id,
            last_serial: self.client.last_serial(),
            reason: error.is_some() as _,
            explanation: error,
        });
    }

    pub fn send_seat(&self, seat: &EiSeat) {
        self.client.event(Seat {
            self_id: self.id,
            seat: seat.id,
            version: seat.version.0,
        });
    }

    #[expect(dead_code)]
    pub fn send_ping(&self, ping: &EiPingpong, version: u32) {
        self.client.event(Ping {
            self_id: self.id,
            ping: ping.id,
            version,
        });
    }

    pub fn announce_seat(&self, seat: &Rc<WlSeatGlobal>) {
        let version = self.client.versions.ei_seat.version.get();
        if version == EiVersion(0) {
            return;
        }
        let kb_state_id = match self.context() {
            EiContext::Sender => seat.seat_kb_state().borrow().id,
            EiContext::Receiver => seat.latest_kb_state().borrow().id,
        };
        let seat = Rc::new(EiSeat {
            id: self.client.new_id(),
            client: self.client.clone(),
            tracker: Default::default(),
            version,
            seat: seat.clone(),
            capabilities: Cell::new(0),
            kb_state_id: Cell::new(kb_state_id),
            keyboard_id: self.client.state.physical_keyboard_ids.next(),
            device: Default::default(),
            pointer: Default::default(),
            pointer_absolute: Default::default(),
            keyboard: Default::default(),
            button: Default::default(),
            scroll: Default::default(),
            touchscreen: Default::default(),
        });
        track!(self.client, seat);
        self.client.add_server_obj(&seat);
        self.send_seat(&seat);
        let v = &self.client.versions;
        let caps = [
            (EI_CAP_POINTER, EiPointer, &v.ei_pointer),
            (
                EI_CAP_POINTER_ABSOLUTE,
                EiPointerAbsolute,
                &v.ei_pointer_absolute,
            ),
            (EI_CAP_SCROLL, EiScroll, &v.ei_scroll),
            (EI_CAP_BUTTON, EiButton, &v.ei_button),
            (EI_CAP_KEYBOARD, EiKeyboard, &v.ei_keyboard),
            (EI_CAP_TOUCHSCREEN, EiTouchscreen, &v.ei_touchscreen),
        ];
        for (mask, interface, version) in caps {
            if version.version.get() > EiVersion(0) {
                seat.send_capability(interface, mask);
            }
        }
        seat.send_name(&seat.seat.seat_name());
        seat.send_done();
        seat.seat.add_ei_seat(&seat);
    }
}

impl EiConnectionRequestHandler for EiConnection {
    type Error = EiConnectionError;

    fn sync(
        &self,
        req: crate::wire_ei::ei_connection::Sync,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let version = EiVersion(req.version);
        if version > self.client.versions.ei_callback.version.get() {
            return Err(EiConnectionError::CallbackVersion(req.version));
        }
        let cb = Rc::new(EiCallback {
            id: req.callback,
            client: self.client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(self.client, cb);
        self.client.add_client_obj(&cb)?;
        cb.send_done(0);
        self.client.remove_obj(&*cb)?;
        Ok(())
    }

    fn disconnect(&self, _req: Disconnect, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.disconnect_announced.set(true);
        self.client.state.ei_clients.shutdown(self.client.id);
        Ok(())
    }
}

ei_object_base! {
    self = EiConnection;
    version = self.version;
}

impl EiObject for EiConnection {}

#[derive(Debug, Error)]
pub enum EiConnectionError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
    #[error("The callback version is too large: {0}")]
    CallbackVersion(u32),
}
efrom!(EiConnectionError, EiClientError);
