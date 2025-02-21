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
        wire::{ZwlrDataControlSourceV1Id, zwlr_data_control_source_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    uapi::OwnedFd,
};

pub struct TestDataControlSource {
    pub id: ZwlrDataControlSourceV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub cancelled: Cell<bool>,
    pub sends: TEEH<(String, Rc<OwnedFd>)>,
}

impl TestDataControlSource {
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn offer(&self, mime_type: &str) -> TestResult {
        self.tran.send(Offer {
            self_id: self.id,
            mime_type,
        })?;
        Ok(())
    }

    fn handle_send(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Send::parse_full(parser)?;
        self.sends.push((ev.mime_type.to_string(), ev.fd));
        Ok(())
    }

    fn handle_cancelled(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Cancelled::parse_full(parser)?;
        self.cancelled.set(true);
        Ok(())
    }
}

impl Drop for TestDataControlSource {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestDataControlSource, ZwlrDataControlSourceV1;

    SEND => handle_send,
    CANCELLED => handle_cancelled,
}

impl TestObject for TestDataControlSource {}
