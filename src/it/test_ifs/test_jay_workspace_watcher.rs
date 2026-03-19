use crate::it::test_error::TestError;
use crate::it::test_ifs::test_jay_workspace::TestJayWorkspace;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::it::testrun::ParseFull;
use crate::utils::buffd::MsgParser;
use crate::wire::JayWorkspaceWatcherId;
use crate::wire::jay_workspace_watcher::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestJayWorkspaceWatcher {
    pub id: JayWorkspaceWatcherId,
    pub tran: Rc<TestTransport>,
    pub workspaces: RefCell<Vec<Rc<TestJayWorkspace>>>,
}

impl TestJayWorkspaceWatcher {
    pub fn workspace_by_name(&self, name: &str) -> Option<Rc<TestJayWorkspace>> {
        self.workspaces
            .borrow()
            .iter()
            .find(|workspace| workspace.name.borrow().as_deref() == Some(name))
            .cloned()
    }

    pub fn live_workspace_by_name(&self, name: &str) -> Option<Rc<TestJayWorkspace>> {
        self.workspaces
            .borrow()
            .iter()
            .find(|workspace| {
                workspace.name.borrow().as_deref() == Some(name) && !workspace.destroyed.get()
            })
            .cloned()
    }

    fn handle_new(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = New::parse_full(parser)?;
        let ws = Rc::new(TestJayWorkspace {
            id: ev.id,
            destroyed: Cell::new(false),
            linear_id: Cell::new(Some(ev.linear_id)),
            name: Default::default(),
            output: Default::default(),
            visible: Default::default(),
        });
        self.tran.add_obj(ws.clone())?;
        self.workspaces.borrow_mut().push(ws);
        Ok(())
    }
}

test_object! {
    TestJayWorkspaceWatcher, JayWorkspaceWatcher;

    NEW => handle_new,
}

impl TestObject for TestJayWorkspaceWatcher {}
