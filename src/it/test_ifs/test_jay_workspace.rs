use crate::it::test_error::TestError;
use crate::it::test_object::TestObject;
use crate::it::testrun::ParseFull;
use crate::utils::buffd::MsgParser;
use crate::wire::JayWorkspaceId;
use crate::wire::jay_workspace::*;
use std::cell::Cell;
use std::cell::RefCell;

pub struct TestJayWorkspace {
    pub id: JayWorkspaceId,
    pub destroyed: Cell<bool>,
    pub linear_id: Cell<Option<u32>>,
    pub name: RefCell<Option<String>>,
    pub output: Cell<Option<u32>>,
    pub visible: Cell<Option<bool>>,
}

impl TestJayWorkspace {
    fn handle_linear_id(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = LinearId::parse_full(parser)?;
        self.linear_id.set(Some(ev.linear_id));
        Ok(())
    }

    fn handle_name(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Name::parse_full(parser)?;
        *self.name.borrow_mut() = Some(ev.name.to_string());
        Ok(())
    }

    fn handle_destroyed(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Destroyed::parse_full(parser)?;
        self.destroyed.set(true);
        Ok(())
    }

    fn handle_done(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Done::parse_full(parser)?;
        Ok(())
    }

    fn handle_output(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Output::parse_full(parser)?;
        self.output.set(Some(ev.global_name));
        Ok(())
    }

    fn handle_visible(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Visible::parse_full(parser)?;
        self.visible.set(Some(ev.visible));
        Ok(())
    }
}

test_object! {
    TestJayWorkspace, JayWorkspace;

    LINEAR_ID => handle_linear_id,
    NAME => handle_name,
    DESTROYED => handle_destroyed,
    DONE => handle_done,
    OUTPUT => handle_output,
    VISIBLE => handle_visible,
}

impl TestObject for TestJayWorkspace {}
