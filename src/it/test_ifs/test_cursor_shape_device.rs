use crate::it::test_error::TestResult;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::wire::WpCursorShapeDeviceV1Id;
use crate::wire::wp_cursor_shape_device_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestCursorShapeDevice {
    pub id: WpCursorShapeDeviceV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestCursorShapeDevice {
    #[expect(dead_code)]
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn set_shape(&self, serial: u32, shape: u32) -> TestResult {
        self.tran.send(SetShape {
            self_id: self.id,
            serial,
            shape,
        })?;
        Ok(())
    }
}

test_object! {
    TestCursorShapeDevice, WpCursorShapeDeviceV1;
}

impl TestObject for TestCursorShapeDevice {}
