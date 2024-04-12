use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{test_seat::TestSeat, test_virtual_keyboard::TestVirtualKeyboard},
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{zwp_virtual_keyboard_manager_v1::*, ZwpVirtualKeyboardManagerV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestVirtualKeyboardManager {
    pub id: ZwpVirtualKeyboardManagerV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestVirtualKeyboardManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
        }
    }

    pub fn create_virtual_keyboard(&self, seat: &TestSeat) -> TestResult<Rc<TestVirtualKeyboard>> {
        let obj = Rc::new(TestVirtualKeyboard {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(CreateVirtualKeyboard {
            self_id: self.id,
            seat: seat.id,
            id: obj.id,
        })?;
        Ok(obj)
    }
}

test_object! {
    TestVirtualKeyboardManager, ZwpVirtualKeyboardManagerV1;
}

impl TestObject for TestVirtualKeyboardManager {}
