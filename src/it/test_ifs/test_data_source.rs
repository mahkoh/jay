use {
    crate::{
        it::{
            test_error::TestResult, test_object::TestObject, test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{wl_data_source::*, WlDataSourceId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestDataSource {
    pub id: WlDataSourceId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestDataSource {
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn offer(&self, mime_type: &str) -> TestResult {
        self.tran.send(Offer {
            self_id: self.id,
            mime_type,
        })?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_actions(&self, actions: u32) -> TestResult {
        self.tran.send(SetActions {
            self_id: self.id,
            dnd_actions: actions,
        })?;
        Ok(())
    }

    fn handle_target(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Target::parse_full(parser)?;
        Ok(())
    }

    fn handle_send(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Send::parse_full(parser)?;
        Ok(())
    }

    fn handle_cancelled(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Cancelled::parse_full(parser)?;
        Ok(())
    }

    fn handle_dnd_drop_performed(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = DndDropPerformed::parse_full(parser)?;
        Ok(())
    }

    fn handle_dnd_finished(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = DndFinished::parse_full(parser)?;
        Ok(())
    }

    fn handle_action(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Action::parse_full(parser)?;
        Ok(())
    }
}

impl Drop for TestDataSource {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestDataSource, WlDataSource;

    TARGET => handle_target,
    SEND => handle_send,
    CANCELLED => handle_cancelled,
    DND_DROP_PERFORMED => handle_dnd_drop_performed,
    DND_FINISHED => handle_dnd_finished,
    ACTION => handle_action,
}

impl TestObject for TestDataSource {}
