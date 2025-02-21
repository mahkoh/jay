use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{ExtForeignToplevelHandleV1Id, ext_foreign_toplevel_handle_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestExtForeignToplevelHandle {
    pub id: ExtForeignToplevelHandleV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub closed: Cell<bool>,
    pub title: Cell<Option<String>>,
    pub app_id: Cell<Option<String>>,
    pub identifier: Cell<Option<String>>,
}

impl TestExtForeignToplevelHandle {
    fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    fn handle_closed(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Closed::parse_full(parser)?;
        self.closed.set(true);
        self.destroy()?;
        Ok(())
    }

    fn handle_done(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Done::parse_full(parser)?;
        Ok(())
    }

    fn handle_title(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Title::parse_full(parser)?;
        self.title.set(Some(ev.title.to_string()));
        Ok(())
    }

    fn handle_app_id(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = AppId::parse_full(parser)?;
        self.app_id.set(Some(ev.app_id.to_string()));
        Ok(())
    }

    fn handle_identifier(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Identifier::parse_full(parser)?;
        self.identifier.set(Some(ev.identifier.to_string()));
        Ok(())
    }
}

test_object! {
    TestExtForeignToplevelHandle, ExtForeignToplevelHandleV1;

    CLOSED => handle_closed,
    DONE => handle_done,
    TITLE => handle_title,
    APP_ID => handle_app_id,
    IDENTIFIER => handle_identifier,
}

impl TestObject for TestExtForeignToplevelHandle {}
