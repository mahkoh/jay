use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_ifs::test_viewport::TestViewport;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::wire::WpViewporterId;
use crate::wire::wp_viewporter::*;
use std::rc::Rc;

pub struct TestViewporter {
    pub id: WpViewporterId,
    pub tran: Rc<TestTransport>,
}

impl TestViewporter {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
        }
    }

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
