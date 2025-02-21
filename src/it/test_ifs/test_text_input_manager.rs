use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{test_seat::TestSeat, test_text_input::TestTextInput},
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{ZwpTextInputManagerV3Id, zwp_text_input_manager_v3::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestTextInputManager {
    pub id: ZwpTextInputManagerV3Id,
    pub tran: Rc<TestTransport>,
}

impl TestTextInputManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
        }
    }

    pub fn get_text_input(&self, seat: &TestSeat) -> TestResult<Rc<TestTextInput>> {
        let obj = Rc::new(TestTextInput {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            enter: Rc::new(Default::default()),
            leave: Rc::new(Default::default()),
            commit_string: Rc::new(Default::default()),
            done: Rc::new(Default::default()),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetTextInput {
            self_id: self.id,
            id: obj.id,
            seat: seat.id,
        })?;
        Ok(obj)
    }
}

test_object! {
    TestTextInputManager, ZwpTextInputManagerV3;
}

impl TestObject for TestTextInputManager {}
