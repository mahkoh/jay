use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{test_surface::TestSurface, test_viewport::TestViewport},
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{wp_viewporter::*, WpViewporterId},
    },
    std::rc::Rc,
};

pub struct TestViewporter {
    pub id: WpViewporterId,
    pub tran: Rc<TestTransport>,
}

impl TestViewporter {
    pub fn get_viewport(&self, surface: &TestSurface) -> TestResult<Rc<TestViewport>> {
        let obj = Rc::new(TestViewport {
            id: self.tran.id(),
            tran: self.tran.clone(),
        });
        self.tran.send(GetViewport {
            self_id: self.id,
            id: obj.id,
            surface: surface.id,
        })?;
        self.tran.add_obj(obj.clone())?;
        Ok(obj)
    }
}

test_object! {
    TestViewporter, WpViewporter;
}

impl TestObject for TestViewporter {}
