use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_seat::tablet::{
            zwp_tablet_seat_v2::ZwpTabletSeatV2, TabletPadStrip, TabletStripEventSource,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_tablet_pad_strip_v2::*, ZwpTabletPadStripV2Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpTabletPadStripV2 {
    pub id: ZwpTabletPadStripV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub seat: Rc<ZwpTabletSeatV2>,
    pub strip: Rc<TabletPadStrip>,
}

impl ZwpTabletPadStripV2 {
    pub fn detach(&self) {
        self.strip.bindings.remove(&self.seat);
    }

    pub fn send_source(&self, source: TabletStripEventSource) {
        self.client.event(Source {
            self_id: self.id,
            source: match source {
                TabletStripEventSource::Finger => 1,
            },
        });
    }

    pub fn send_position(&self, position: u32) {
        self.client.event(Position {
            self_id: self.id,
            position,
        });
    }

    pub fn send_stop(&self) {
        self.client.event(Stop { self_id: self.id });
    }

    pub fn send_frame(&self, time: u32) {
        self.client.event(Frame {
            self_id: self.id,
            time,
        });
    }
}

impl ZwpTabletPadStripV2RequestHandler for ZwpTabletPadStripV2 {
    type Error = ZwpTabletPadStripV2Error;

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
    self = ZwpTabletPadStripV2;
    version = self.version;
}

impl Object for ZwpTabletPadStripV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpTabletPadStripV2);

#[derive(Debug, Error)]
pub enum ZwpTabletPadStripV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTabletPadStripV2Error, ClientError);
