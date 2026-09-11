use crate::it::test_client::TestClient;
use crate::video::drm::syncobj::Syncobj;
use crate::wire::WpLinuxDrmSyncobjTimelineV1Id;
use std::rc::Rc;

pub struct TestSyncobjTimeline {
    pub id: WpLinuxDrmSyncobjTimelineV1Id,
}

impl TestClient {
    pub fn import_syncobj_timeline(&self, syncobj: &Syncobj) -> Rc<TestSyncobjTimeline> {
        let id = self
            .client
            .send_wp_linux_drm_syncobj_manager_v1_import_timeline(syncobj.fd());
        Rc::new(TestSyncobjTimeline { id })
    }
}
