use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_registry::TestRegistry;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::wire::WlFixesId;
use crate::wire::wl_fixes::*;
use std::rc::Rc;

pub struct TestWlFixes {
    pub id: WlFixesId,
    pub tran: Rc<TestTransport>,
}

impl TestWlFixes {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
        }
    }

    pub fn destroy_registry(&self, registry: &TestRegistry) -> TestResult {
        self.tran.send(DestroyRegistry {
            self_id: self.id,
            registry: registry.id,
        })?;
        Ok(())
    }
}

test_object! {
    TestWlFixes, WlFixes;
}

impl TestObject for TestWlFixes {}
