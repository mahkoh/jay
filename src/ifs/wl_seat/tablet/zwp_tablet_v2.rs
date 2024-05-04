use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_seat::tablet::{zwp_tablet_seat_v2::ZwpTabletSeatV2, Tablet},
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_tablet_v2::*, ZwpTabletV2Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpTabletV2 {
    pub id: ZwpTabletV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub seat: Rc<ZwpTabletSeatV2>,
    pub tablet: Rc<Tablet>,
}

impl ZwpTabletV2 {
    fn detach(&self) {
        self.tablet.bindings.remove(&self.seat);
    }

    pub fn send_name(&self, name: &str) {
        self.client.event(Name {
            self_id: self.id,
            name,
        });
    }

    pub fn send_id(&self, vid: u32, pid: u32) {
        self.client.event(Id {
            self_id: self.id,
            vid,
            pid,
        });
    }

    pub fn send_path(&self, path: &str) {
        self.client.event(Path {
            self_id: self.id,
            path,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_removed(&self) {
        self.client.event(Removed { self_id: self.id });
    }
}

impl ZwpTabletV2RequestHandler for ZwpTabletV2 {
    type Error = ZwpTabletV2Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpTabletV2;
    version = self.version;
}

impl Object for ZwpTabletV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpTabletV2);

#[derive(Debug, Error)]
pub enum ZwpTabletV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTabletV2Error, ClientError);
