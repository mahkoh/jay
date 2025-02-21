use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{test_input_method::TestInputMethod, test_seat::TestSeat},
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{ZwpInputMethodManagerV2Id, zwp_input_method_manager_v2::GetInputMethod},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestInputMethodManager {
    pub id: ZwpInputMethodManagerV2Id,
    pub tran: Rc<TestTransport>,
}

impl TestInputMethodManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
        }
    }

    pub fn get_input_method(&self, seat: &TestSeat) -> TestResult<Rc<TestInputMethod>> {
        let obj = Rc::new(TestInputMethod {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            activate: Rc::new(Default::default()),
            done: Rc::new(Default::default()),
            done_received: Default::default(),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetInputMethod {
            self_id: self.id,
            seat: seat.id,
            input_method: obj.id,
        })?;
        Ok(obj)
    }
}

test_object! {
    TestInputMethodManager, ZwpInputMethodManagerV2;
}

impl TestObject for TestInputMethodManager {}
