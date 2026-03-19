use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_ext_workspace_group_handle::TestExtWorkspaceGroupHandle;
use crate::it::test_ifs::test_ext_workspace_handle::TestExtWorkspaceHandle;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::it::testrun::ParseFull;
use crate::object::ObjectId;
use crate::utils::buffd::MsgParser;
use crate::wire::ExtWorkspaceManagerV1Id;
use crate::wire::ext_workspace_manager_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestExtWorkspaceManager {
    pub id: ExtWorkspaceManagerV1Id,
    pub tran: Rc<TestTransport>,
    pub finished: Cell<bool>,
    pub done_count: Cell<u32>,
    pub groups: RefCell<Vec<Rc<TestExtWorkspaceGroupHandle>>>,
    pub workspaces: RefCell<Vec<Rc<TestExtWorkspaceHandle>>>,
}

impl TestExtWorkspaceManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            finished: Cell::new(false),
            done_count: Cell::new(0),
            groups: Default::default(),
            workspaces: Default::default(),
        }
    }

    pub fn commit(&self) -> TestResult {
        self.tran.send(Commit { self_id: self.id })?;
        Ok(())
    }

    pub fn workspace_by_name(&self, name: &str) -> Option<Rc<TestExtWorkspaceHandle>> {
        self.workspaces
            .borrow()
            .iter()
            .find(|workspace| workspace.name.borrow().as_deref() == Some(name))
            .cloned()
    }

    pub fn workspace_by_id(&self, id: impl Into<ObjectId>) -> Option<Rc<TestExtWorkspaceHandle>> {
        let id = id.into();
        self.workspaces
            .borrow()
            .iter()
            .find(|workspace| {
                let workspace_id: ObjectId = workspace.id.into();
                workspace_id == id
            })
            .cloned()
    }

    fn handle_workspace_group(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = WorkspaceGroup::parse_full(parser)?;
        let group = Rc::new(TestExtWorkspaceGroupHandle {
            id: ev.workspace_group,
            manager: Rc::downgrade(self),
            removed: Cell::new(false),
            capabilities: Cell::new(0),
            outputs: Default::default(),
            workspaces: Default::default(),
        });
        self.tran.add_obj(group.clone())?;
        self.groups.borrow_mut().push(group);
        Ok(())
    }

    fn handle_workspace(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Workspace::parse_full(parser)?;
        let workspace = Rc::new(TestExtWorkspaceHandle {
            id: ev.workspace,
            tran: self.tran.clone(),
            removed: Cell::new(false),
            state: Cell::new(0),
            capabilities: Cell::new(0),
            identifier: Default::default(),
            name: Default::default(),
            current_group: Cell::new(None),
        });
        self.tran.add_obj(workspace.clone())?;
        self.workspaces.borrow_mut().push(workspace);
        Ok(())
    }

    fn handle_done(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Done::parse_full(parser)?;
        self.done_count.set(self.done_count.get() + 1);
        Ok(())
    }

    fn handle_finished(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Finished::parse_full(parser)?;
        self.finished.set(true);
        Ok(())
    }
}

test_object! {
    TestExtWorkspaceManager, ExtWorkspaceManagerV1;

    WORKSPACE_GROUP => handle_workspace_group,
    WORKSPACE => handle_workspace,
    DONE => handle_done,
    FINISHED => handle_finished,
}

impl TestObject for TestExtWorkspaceManager {}
