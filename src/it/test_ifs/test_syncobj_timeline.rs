use {
    crate::{
        it::{test_error::TestError, test_object::TestObject, test_transport::TestTransport},
        wire::{WpLinuxDrmSyncobjTimelineV1Id, wp_linux_drm_syncobj_timeline_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

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
