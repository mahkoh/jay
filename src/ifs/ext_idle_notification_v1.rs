use crate::async_engine::SpawnedFuture;
use crate::client::Client;
use crate::client::ClientError;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::utils::asyncevent::AsyncEvent;
use crate::wire::ExtIdleNotificationV1Id;
use crate::wire::ext_idle_notification_v1::*;
use std::cell::Cell;
use std::rc::Rc;
use thiserror::Error;

pub struct ExtIdleNotificationV1 {
    pub id: ExtIdleNotificationV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub resume: AsyncEvent,
    pub task: Cell<Option<SpawnedFuture<()>>>,
    pub seat: Rc<WlSeatGlobal>,
    pub duration_usec: u64,
    pub version: Version,
}

impl ExtIdleNotificationV1 {
    fn detach(&self) {
        self.seat.remove_idle_notification(self);
        self.client.state.idle.remove_inhibited_notification(self);
        self.task.take();
    }
}

impl ExtIdleNotificationV1RequestHandler for ExtIdleNotificationV1 {
    type Error = ExtIdleNotificationV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl ExtIdleNotificationV1 {
    pub fn send_idled(&self) {
        self.client.event(Idled { self_id: self.id });
    }

    pub fn send_resumed(&self) {
        self.client.event(Resumed { self_id: self.id });
    }
}

object_base! {
    self = ExtIdleNotificationV1;
    version = self.version;
}

impl Object for ExtIdleNotificationV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(ExtIdleNotificationV1);

#[derive(Debug, Error)]
pub enum ExtIdleNotificationV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtIdleNotificationV1Error, ClientError);
