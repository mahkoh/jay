use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        video::drm::sync_obj::SyncObj,
        wire::{wp_linux_drm_syncobj_timeline_v1::*, WpLinuxDrmSyncobjTimelineV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpLinuxDrmSyncobjTimelineV1 {
    id: WpLinuxDrmSyncobjTimelineV1Id,
    client: Rc<Client>,
    pub sync_obj: Rc<SyncObj>,
    pub tracker: Tracker<Self>,
    version: Version,
}

impl WpLinuxDrmSyncobjTimelineV1 {
    pub fn new(
        id: WpLinuxDrmSyncobjTimelineV1Id,
        client: &Rc<Client>,
        sync_obj: &Rc<SyncObj>,
        version: Version,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            sync_obj: sync_obj.clone(),
            version,
        }
    }
}

impl WpLinuxDrmSyncobjTimelineV1RequestHandler for WpLinuxDrmSyncobjTimelineV1 {
    type Error = WpLinuxDrmSyncobjTimelineV1Error;

    fn destroy(&self, _destroy: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpLinuxDrmSyncobjTimelineV1;
    version = self.version;
}

impl Object for WpLinuxDrmSyncobjTimelineV1 {}

dedicated_add_obj!(
    WpLinuxDrmSyncobjTimelineV1,
    WpLinuxDrmSyncobjTimelineV1Id,
    timelines
);

#[derive(Debug, Error)]
pub enum WpLinuxDrmSyncobjTimelineV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpLinuxDrmSyncobjTimelineV1Error, ClientError);
