use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{ExtTransientSeatV1Id, ext_transient_seat_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtTransientSeatV1 {
    pub id: ExtTransientSeatV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ExtTransientSeatV1 {
    pub fn send_denied(&self) {
        self.client.event(Denied { self_id: self.id });
    }
}

impl ExtTransientSeatV1RequestHandler for ExtTransientSeatV1 {
    type Error = ExtTransientSeatV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ExtTransientSeatV1;
    version = self.version;
}

impl Object for ExtTransientSeatV1 {}

simple_add_obj!(ExtTransientSeatV1);

#[derive(Debug, Error)]
pub enum ExtTransientSeatV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtTransientSeatV1Error, ClientError);
