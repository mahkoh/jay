use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::video::drm::syncobj::Syncobj;
use crate::wire::WpLinuxDrmSyncobjTimelineV1Id;
use crate::wire::wp_linux_drm_syncobj_timeline_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
pub struct WpLinuxDrmSyncobjTimelineV1 {
    id: WpLinuxDrmSyncobjTimelineV1Id,
    client: Rc<Client>,
    pub syncobj: Rc<Syncobj>,
    pub tracker: Tracker<Self>,
    version: Version,
}

impl WpLinuxDrmSyncobjTimelineV1 {
    pub fn new(
        id: WpLinuxDrmSyncobjTimelineV1Id,
        client: &Rc<Client>,
        syncobj: &Rc<Syncobj>,
        version: Version,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            syncobj: syncobj.clone(),
            version,
        }
    }
}

impl WpLinuxDrmSyncobjTimelineV1RequestHandler for WpLinuxDrmSyncobjTimelineV1 {
    type Error = Infallible;

    fn destroy(&self, _destroy: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
