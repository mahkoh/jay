use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_jay_workspace::TestJayWorkspace,
            test_object::TestObject, test_transport::TestTransport, testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{JayWorkspaceWatcherId, jay_workspace_watcher::*},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

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
