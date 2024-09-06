use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{test_dmabuf_feedback::TestDmabufFeedback, test_surface::TestSurface},
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{zwp_linux_dmabuf_v1::*, ZwpLinuxDmabufV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestDmabuf {
    pub id: ZwpLinuxDmabufV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestDmabuf {
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

    #[expect(dead_code)]
    pub fn get_default_feedback(&self) -> TestResult<Rc<TestDmabufFeedback>> {
        let obj = Rc::new(TestDmabufFeedback::new(&self.tran));
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetDefaultFeedback {
            self_id: self.id,
            id: obj.id,
        })?;
        Ok(obj)
    }

    pub fn get_surface_feedback(
        &self,
        surface: &TestSurface,
    ) -> TestResult<Rc<TestDmabufFeedback>> {
        let obj = Rc::new(TestDmabufFeedback::new(&self.tran));
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetSurfaceFeedback {
            self_id: self.id,
            id: obj.id,
            surface: surface.id,
        })?;
        Ok(obj)
    }
}

impl Drop for TestDmabuf {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestDmabuf, ZwpLinuxDmabufV1;
}

impl TestObject for TestDmabuf {}
