use {
    crate::{
        it::{test_error::TestResult, test_object::TestObject, test_transport::TestTransport},
        wire::{wp_cursor_shape_device_v1::*, WpCursorShapeDeviceV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

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
