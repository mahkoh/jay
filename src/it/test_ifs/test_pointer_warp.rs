use crate::fixed::Fixed;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_pointer::TestPointer;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::wire::WpPointerWarpV1Id;
use crate::wire::wp_pointer_warp_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestPointerWarp {
    pub id: WpPointerWarpV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestPointerWarp {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
        }
    }

    #[expect(dead_code)]
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn warp_pointer(
        &self,
        surface: &TestSurface,
        pointer: &TestPointer,
        x: Fixed,
        y: Fixed,
        serial: u32,
    ) -> TestResult {
        self.tran.send(WarpPointer {
            self_id: self.id,
            surface: surface.id,
            pointer: pointer.id,
            x,
            y,
            serial,
        })?;
        Ok(())
    }
}

test_object! {
    TestPointerWarp, WpPointerWarpV1;
}

impl TestObject for TestPointerWarp {}
