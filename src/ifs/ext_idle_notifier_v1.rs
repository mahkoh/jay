use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::ext_idle_notification_v1::ExtIdleNotificationV1,
        leaks::Tracker,
        object::Object,
        time::now_usec,
        utils::{
            buffd::{MsgParser, MsgParserError},
            errorfmt::ErrorFmt,
        },
        wire::{ext_idle_notifier_v1::*, ExtIdleNotifierV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ExtIdleNotifierV1Global {
    pub name: GlobalName,
}

impl ExtIdleNotifierV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtIdleNotifierV1Id,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), ExtIdleNotifierV1Error> {
        let obj = Rc::new(ExtIdleNotifierV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

pub struct ExtIdleNotifierV1 {
    pub id: ExtIdleNotifierV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl ExtIdleNotifierV1 {
    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtIdleNotifierV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_idle_notification(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtIdleNotifierV1Error> {
        let req: GetIdleNotification = self.client.parse(self, msg)?;
        let seat = self.client.lookup(req.seat)?;
        let notification = Rc::new(ExtIdleNotificationV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            resume: Default::default(),
            task: Cell::new(None),
            seat: seat.global.clone(),
            duration_usec: (req.timeout as u64).max(1000).saturating_mul(1000),
        });
        self.client.add_client_obj(&notification)?;
        let future = self.client.state.eng.spawn(run(notification.clone()));
        notification.task.set(Some(future));
        Ok(())
    }
}

async fn run(n: Rc<ExtIdleNotificationV1>) {
    loop {
        let now = now_usec();
        let elapsed = now.saturating_sub(n.seat.last_input());
        if elapsed < n.duration_usec {
            let res = n
                .client
                .state
                .wheel
                .timeout((n.duration_usec - elapsed + 999) / 1000)
                .await;
            if let Err(e) = res {
                log::error!("Could not wait for idle timeout to elapse: {}", ErrorFmt(e));
                return;
            }
        } else {
            n.send_idled();
            n.seat.add_idle_notification(&n);
            n.resume.triggered().await;
            n.send_resumed();
        }
    }
}

global_base!(
    ExtIdleNotifierV1Global,
    ExtIdleNotifierV1,
    ExtIdleNotifierV1Error
);

impl Global for ExtIdleNotifierV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn secure(&self) -> bool {
        true
    }
}

simple_add_global!(ExtIdleNotifierV1Global);

object_base! {
    self = ExtIdleNotifierV1;

    DESTROY => destroy,
    GET_IDLE_NOTIFICATION => get_idle_notification,
}

impl Object for ExtIdleNotifierV1 {}

simple_add_obj!(ExtIdleNotifierV1);

#[derive(Debug, Error)]
pub enum ExtIdleNotifierV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtIdleNotifierV1Error, MsgParserError);
efrom!(ExtIdleNotifierV1Error, ClientError);
