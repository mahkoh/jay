use crate::it::test_error::TestError;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::wire::WpLinuxDrmSyncobjTimelineV1Id;
use crate::wire::wp_linux_drm_syncobj_timeline_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestSyncobjTimeline {
    pub id: WpLinuxDrmSyncobjTimelineV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestSyncobjTimeline {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }
}

impl Drop for TestSyncobjTimeline {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestSyncobjTimeline, WpLinuxDrmSyncobjTimelineV1;
}

impl TestObject for TestSyncobjTimeline {}
