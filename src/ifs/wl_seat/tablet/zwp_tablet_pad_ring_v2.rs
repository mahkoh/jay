use {
    crate::{
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::wl_seat::tablet::{
            TabletPadRing, TabletRingEventSource, zwp_tablet_seat_v2::ZwpTabletSeatV2,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpTabletPadRingV2Id, zwp_tablet_pad_ring_v2::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpTabletPadRingV2 {
    pub id: ZwpTabletPadRingV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub seat: Rc<ZwpTabletSeatV2>,
    pub ring: Rc<TabletPadRing>,
}

impl ZwpTabletPadRingV2 {
    pub fn detach(&self) {
        self.ring.bindings.remove(&self.seat);
    }

    pub fn send_source(&self, source: TabletRingEventSource) {
        self.client.event(Source {
            self_id: self.id,
            source: match source {
                TabletRingEventSource::Finger => 1,
            },
        });
    }

    pub fn send_angle(&self, degrees: Fixed) {
        self.client.event(Angle {
            self_id: self.id,
            degrees,
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

impl ZwpTabletPadRingV2RequestHandler for ZwpTabletPadRingV2 {
    type Error = ZwpTabletPadRingV2Error;

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
    self = ZwpTabletPadRingV2;
    version = self.version;
}

impl Object for ZwpTabletPadRingV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpTabletPadRingV2);

#[derive(Debug, Error)]
pub enum ZwpTabletPadRingV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTabletPadRingV2Error, ClientError);
