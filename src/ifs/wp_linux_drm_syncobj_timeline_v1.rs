use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
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
}

impl WpLinuxDrmSyncobjTimelineV1 {
    pub fn new(
        id: WpLinuxDrmSyncobjTimelineV1Id,
        client: &Rc<Client>,
        sync_obj: &Rc<SyncObj>,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            sync_obj: sync_obj.clone(),
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WpLinuxDrmSyncobjTimelineV1Error> {
        let _destroy: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpLinuxDrmSyncobjTimelineV1;

    DESTROY => destroy,
}

impl Object for WpLinuxDrmSyncobjTimelineV1 {}

dedicated_add_obj!(
    WpLinuxDrmSyncobjTimelineV1,
    WpLinuxDrmSyncobjTimelineV1Id,
    timelines
);

#[derive(Debug, Error)]
pub enum WpLinuxDrmSyncobjTimelineV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpLinuxDrmSyncobjTimelineV1Error, MsgParserError);
efrom!(WpLinuxDrmSyncobjTimelineV1Error, ClientError);
