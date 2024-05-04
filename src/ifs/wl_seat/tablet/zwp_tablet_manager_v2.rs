use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_seat::tablet::zwp_tablet_seat_v2::ZwpTabletSeatV2,
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_tablet_manager_v2::*, ZwpTabletManagerV2Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpTabletManagerV2Global {
    pub name: GlobalName,
}

pub struct ZwpTabletManagerV2 {
    pub id: ZwpTabletManagerV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpTabletManagerV2Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpTabletManagerV2Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwpTabletManagerV2Error> {
        let obj = Rc::new(ZwpTabletManagerV2 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    ZwpTabletManagerV2Global,
    ZwpTabletManagerV2,
    ZwpTabletManagerV2Error
);

impl Global for ZwpTabletManagerV2Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpTabletManagerV2Global);

impl ZwpTabletManagerV2RequestHandler for ZwpTabletManagerV2 {
    type Error = ZwpTabletManagerV2Error;

    fn get_tablet_seat(&self, req: GetTabletSeat, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?.global.clone();
        let obj = Rc::new(ZwpTabletSeatV2 {
            id: req.tablet_seat,
            client: self.client.clone(),
            seat: seat.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        seat.tablet_add_seat(&obj);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpTabletManagerV2;
    version = self.version;
}

impl Object for ZwpTabletManagerV2 {}

simple_add_obj!(ZwpTabletManagerV2);

#[derive(Debug, Error)]
pub enum ZwpTabletManagerV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTabletManagerV2Error, ClientError);
