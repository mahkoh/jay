use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ExtTransientSeatV1Id;
use crate::wire::ext_transient_seat_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
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
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
