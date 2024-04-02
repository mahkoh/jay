use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{
                test_surface::TestSurface, test_syncobj_surface::TestSyncobjSurface,
                test_syncobj_timeline::TestSyncobjTimeline,
            },
            test_object::TestObject,
            test_transport::TestTransport,
        },
        video::drm::sync_obj::SyncObj,
        wire::{wp_linux_drm_syncobj_manager_v1::*, WpLinuxDrmSyncobjManagerV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSyncobjManager {
    pub id: WpLinuxDrmSyncobjManagerV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestSyncobjManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
        }
    }

    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn get_surface(&self, surface: &TestSurface) -> TestResult<Rc<TestSyncobjSurface>> {
        let obj = Rc::new(TestSyncobjSurface {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetSurface {
            self_id: self.id,
            id: obj.id,
            surface: surface.id,
        })?;
        Ok(obj)
    }

    pub fn import_timeline(&self, syncobj: &SyncObj) -> TestResult<Rc<TestSyncobjTimeline>> {
        let obj = Rc::new(TestSyncobjTimeline {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(ImportTimeline {
            self_id: self.id,
            id: obj.id,
            fd: syncobj.fd().clone(),
        })?;
        Ok(obj)
    }
}

impl Drop for TestSyncobjManager {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestSyncobjManager, WpLinuxDrmSyncobjManagerV1;
}

impl TestObject for TestSyncobjManager {}
