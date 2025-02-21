use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_object::TestObject,
            test_transport::TestTransport,
            test_utils::test_expected_event::TEEH,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{ZwpTextInputV3Id, zwp_text_input_v3::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestTextInput {
    pub id: ZwpTextInputV3Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub enter: TEEH<Enter>,
    pub leave: TEEH<Leave>,
    pub commit_string: TEEH<String>,
    pub done: TEEH<Done>,
}

impl TestTextInput {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn enable(&self) -> TestResult {
        self.tran.send(Enable { self_id: self.id })
    }

    pub fn disable(&self) -> TestResult {
        self.tran.send(Disable { self_id: self.id })
    }

    pub fn set_cursor_rectangle(&self, x: i32, y: i32, width: i32, height: i32) -> TestResult {
        self.tran.send(SetCursorRectangle {
            self_id: self.id,
            x,
            y,
            width,
            height,
        })
    }

    pub fn commit(&self) -> TestResult {
        self.tran.send(Commit { self_id: self.id })
    }

    fn handle_enter(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Enter::parse_full(parser)?;
        self.enter.push(ev);
        Ok(())
    }

    fn handle_leave(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Leave::parse_full(parser)?;
        self.leave.push(ev);
        Ok(())
    }

    fn handle_commit_string(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = CommitString::parse_full(parser)?;
        self.commit_string
            .push(ev.text.unwrap_or_default().to_string());
        Ok(())
    }

    fn handle_done(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Done::parse_full(parser)?;
        self.done.push(ev);
        Ok(())
    }
}

impl Drop for TestTextInput {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestTextInput, ZwpTextInputV3;

    ENTER => handle_enter,
    LEAVE => handle_leave,
    COMMIT_STRING => handle_commit_string,
    DONE => handle_done,
}

impl TestObject for TestTextInput {}
