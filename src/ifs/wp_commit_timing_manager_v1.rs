use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_commit_timer_v1::{WpCommitTimerV1, WpCommitTimerV1Error},
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WpCommitTimingManagerV1Id,
            wp_commit_timing_manager_v1::{
                Destroy, GetTimer, WpCommitTimingManagerV1RequestHandler,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpCommitTimingManagerV1Global {
    pub name: GlobalName,
}

pub struct WpCommitTimingManagerV1 {
    pub id: WpCommitTimingManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpCommitTimingManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpCommitTimingManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpCommitTimingManagerV1Error> {
        let obj = Rc::new(WpCommitTimingManagerV1 {
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
    WpCommitTimingManagerV1Global,
    WpCommitTimingManagerV1,
    WpCommitTimingManagerV1Error
);

impl Global for WpCommitTimingManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpCommitTimingManagerV1Global);

impl WpCommitTimingManagerV1RequestHandler for WpCommitTimingManagerV1 {
    type Error = WpCommitTimingManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_timer(&self, req: GetTimer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let obj = Rc::new(WpCommitTimerV1::new(req.id, self.version, &surface));
        track!(self.client, obj);
        obj.install()?;
        self.client.add_client_obj(&obj)?;
        Ok(())
    }
}

object_base! {
    self = WpCommitTimingManagerV1;
    version = self.version;
}

impl Object for WpCommitTimingManagerV1 {}

simple_add_obj!(WpCommitTimingManagerV1);

#[derive(Debug, Error)]
pub enum WpCommitTimingManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpCommitTimerV1Error(#[from] WpCommitTimerV1Error),
}
efrom!(WpCommitTimingManagerV1Error, ClientError);
