use {
    crate::{
        it::{
            test_error::TestResult, test_ifs::test_syncobj_timeline::TestSyncobjTimeline,
            test_object::TestObject, test_transport::TestTransport,
        },
        wire::{wp_linux_drm_syncobj_surface_v1::*, WpLinuxDrmSyncobjSurfaceV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSyncobjSurface {
    pub id: WpLinuxDrmSyncobjSurfaceV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestSyncobjSurface {
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn set_acquire_point(&self, tl: &TestSyncobjTimeline, point: u64) -> TestResult {
        self.tran.send(SetAcquirePoint {
            self_id: self.id,
            timeline: tl.id,
            point_hi: (point >> 32) as _,
            point_lo: point as _,
        })?;
        Ok(())
    }

    pub fn set_release_point(&self, tl: &TestSyncobjTimeline, point: u64) -> TestResult {
        self.tran.send(SetReleasePoint {
            self_id: self.id,
            timeline: tl.id,
            point_hi: (point >> 32) as _,
            point_lo: point as _,
        })?;
        Ok(())
    }
}

impl Drop for TestSyncobjSurface {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestSyncobjSurface, WpLinuxDrmSyncobjSurfaceV1;
}

impl TestObject for TestSyncobjSurface {}
