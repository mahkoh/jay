use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_seat::tablet::{
            TabletPadGroup, zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2,
            zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpTabletPadGroupV2Id, zwp_tablet_pad_group_v2::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpTabletPadGroupV2 {
    pub id: ZwpTabletPadGroupV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub seat: Rc<ZwpTabletSeatV2>,
    pub group: Rc<TabletPadGroup>,
}

impl ZwpTabletPadGroupV2 {
    pub fn detach(&self) {
        self.group.bindings.remove(&self.seat);
    }

    pub fn send_buttons(&self, buttons: &[u32]) {
        self.client.event(Buttons {
            self_id: self.id,
            buttons,
        });
    }

    pub fn send_ring(&self, ring: &ZwpTabletPadRingV2) {
        self.client.event(Ring {
            self_id: self.id,
            ring: ring.id,
        });
    }

    pub fn send_strip(&self, strip: &ZwpTabletPadStripV2) {
        self.client.event(Strip {
            self_id: self.id,
            strip: strip.id,
        });
    }

    pub fn send_modes(&self, modes: u32) {
        self.client.event(Modes {
            self_id: self.id,
            modes,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_mode_switch(&self, time: u32, serial: u64, mode: u32) {
        self.client.event(ModeSwitch {
            self_id: self.id,
            time,
            serial: serial as _,
            mode,
        });
    }
}

impl ZwpTabletPadGroupV2RequestHandler for ZwpTabletPadGroupV2 {
    type Error = ZwpTabletPadGroupV2Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpTabletPadGroupV2;
    version = self.version;
}

impl Object for ZwpTabletPadGroupV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpTabletPadGroupV2);

#[derive(Debug, Error)]
pub enum ZwpTabletPadGroupV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTabletPadGroupV2Error, ClientError);
