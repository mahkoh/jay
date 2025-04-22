use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_seat::tablet::{TabletPadDial, zwp_tablet_seat_v2::ZwpTabletSeatV2},
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpTabletPadDialV2Id, zwp_tablet_pad_dial_v2::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpTabletPadDialV2 {
    pub id: ZwpTabletPadDialV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub seat: Rc<ZwpTabletSeatV2>,
    pub dial: Rc<TabletPadDial>,
}

impl ZwpTabletPadDialV2 {
    pub fn detach(&self) {
        self.dial.bindings.remove(&self.seat);
    }

    pub fn send_delta(&self, value120: i32) {
        self.client.event(Delta {
            self_id: self.id,
            value120,
        });
    }

    pub fn send_frame(&self, time: u32) {
        self.client.event(Frame {
            self_id: self.id,
            time,
        });
    }
}

impl ZwpTabletPadDialV2RequestHandler for ZwpTabletPadDialV2 {
    type Error = ZwpTabletPadDialV2Error;

    fn set_feedback(&self, _req: SetFeedback<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpTabletPadDialV2;
    version = self.version;
}

impl Object for ZwpTabletPadDialV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpTabletPadDialV2);

#[derive(Debug, Error)]
pub enum ZwpTabletPadDialV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTabletPadDialV2Error, ClientError);
