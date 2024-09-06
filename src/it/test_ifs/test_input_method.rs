use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_ifs::{
                test_input_method_keyboard_grab::TestInputMethodKeyboardGrab,
                test_input_popup_surface::TestInputPopupSurface, test_surface::TestSurface,
            },
            test_object::TestObject,
            test_transport::TestTransport,
            test_utils::test_expected_event::TEEH,
            testrun::ParseFull,
        },
        utils::{buffd::MsgParser, numcell::NumCell},
        wire::{zwp_input_method_v2::*, ZwpInputMethodV2Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestInputMethod {
    pub id: ZwpInputMethodV2Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub activate: TEEH<bool>,
    pub done: TEEH<()>,
    pub done_received: NumCell<u32>,
}

impl TestInputMethod {
    pub fn commit_string(&self, s: &str) -> TestResult {
        self.tran.send(CommitString {
            self_id: self.id,
            text: s,
        })
    }

    pub fn commit(&self) -> TestResult {
        self.tran.send(Commit {
            self_id: self.id,
            serial: self.done_received.get(),
        })
    }

    #[expect(dead_code)]
    pub fn grab(&self) -> TestResult<Rc<TestInputMethodKeyboardGrab>> {
        let obj = Rc::new(TestInputMethodKeyboardGrab {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            keymap: Rc::new(Default::default()),
            key: Rc::new(Default::default()),
            modifiers: Rc::new(Default::default()),
            repeat_info: Rc::new(Default::default()),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GrabKeyboard {
            self_id: self.id,
            keyboard: obj.id,
        })?;
        Ok(obj)
    }

    pub fn get_popup(&self, surface: &TestSurface) -> TestResult<Rc<TestInputPopupSurface>> {
        let obj = Rc::new(TestInputPopupSurface {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetInputPopupSurface {
            self_id: self.id,
            id: obj.id,
            surface: surface.id,
        })?;
        Ok(obj)
    }

    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    fn handle_activate(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Activate::parse_full(parser)?;
        self.activate.push(true);
        Ok(())
    }

    fn handle_deactivate(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Deactivate::parse_full(parser)?;
        self.activate.push(false);
        Ok(())
    }

    fn handle_done(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Done::parse_full(parser)?;
        self.done.push(());
        self.done_received.fetch_add(1);
        Ok(())
    }
}

impl Drop for TestInputMethod {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestInputMethod, ZwpInputMethodV2;

    ACTIVATE => handle_activate,
    DEACTIVATE => handle_deactivate,
    DONE => handle_done,
}

impl TestObject for TestInputMethod {}
