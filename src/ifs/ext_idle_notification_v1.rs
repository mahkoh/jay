use {
    crate::{
        async_engine::SpawnedFuture,
        client::{Client, ClientError},
        ifs::wl_seat::WlSeatGlobal,
        leaks::Tracker,
        object::Object,
        utils::{
            asyncevent::AsyncEvent,
            buffd::{MsgParser, MsgParserError},
        },
        wire::{ext_idle_notification_v1::*, ExtIdleNotificationV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ExtIdleNotificationV1 {
    pub id: ExtIdleNotificationV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub resume: AsyncEvent,
    pub task: Cell<Option<SpawnedFuture<()>>>,
    pub seat: Rc<WlSeatGlobal>,
    pub duration_usec: u64,
}

impl ExtIdleNotificationV1 {
    fn detach(&self) {
        self.seat.remove_idle_notification(self);
        self.task.take();
    }

    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtIdleNotificationV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    pub fn send_idled(&self) {
        self.client.event(Idled { self_id: self.id });
    }

    pub fn send_resumed(&self) {
        self.client.event(Resumed { self_id: self.id });
    }
}

object_base! {
    self = ExtIdleNotificationV1;

    DESTROY => destroy,
}

impl Object for ExtIdleNotificationV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ExtIdleNotificationV1);

#[derive(Debug, Error)]
pub enum ExtIdleNotificationV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtIdleNotificationV1Error, MsgParserError);
efrom!(ExtIdleNotificationV1Error, ClientError);
