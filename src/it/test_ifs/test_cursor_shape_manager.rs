use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{
                test_cursor_shape_device::TestCursorShapeDevice, test_pointer::TestPointer,
            },
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{WpCursorShapeManagerV1Id, wp_cursor_shape_manager_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestCursorShapeManager {
    pub id: WpCursorShapeManagerV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestCursorShapeManager {
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

    pub fn get_pointer(&self, pointer: &TestPointer) -> TestResult<Rc<TestCursorShapeDevice>> {
        let obj = Rc::new(TestCursorShapeDevice {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.send(GetPointer {
            self_id: self.id,
            cursor_shape_device: obj.id,
            pointer: pointer.id,
        })?;
        self.tran.add_obj(obj.clone())?;
        Ok(obj)
    }
}

test_object! {
    TestCursorShapeManager, WpCursorShapeManagerV1;
}

impl TestObject for TestCursorShapeManager {}
