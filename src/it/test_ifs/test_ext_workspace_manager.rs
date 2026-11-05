use crate::client::Client;
use crate::globals::Singleton;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestErrorError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_ext_workspace_group_handle::TestExtWorkspaceGroupHandle;
use crate::it::test_ifs::test_ext_workspace_handle::TestExtWorkspaceHandle;
use crate::wire::ExtWorkspaceManagerV1Id;
use crate::wire::ObjectId;
use crate::wire::ext_workspace_manager_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestExtWorkspaceManager {
    pub id: ExtWorkspaceManagerV1Id,
    pub client: Rc<Client>,
    pub finished: Cell<bool>,
    pub done_count: Cell<u32>,
    pub groups: RefCell<Vec<Rc<TestExtWorkspaceGroupHandle>>>,
    pub workspaces: RefCell<Vec<Rc<TestExtWorkspaceHandle>>>,
}

impl TestExtWorkspaceManager {
    pub fn commit(&self) -> TestResult {
        self.client.request(Commit { self_id: self.id });
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
}

synthetic_event_handler!(TestExtWorkspaceManager);

impl ExtWorkspaceManagerV1EventHandler for TestExtWorkspaceManager {
    type Error = TestErrorError;

    fn workspace_group(&self, ev: WorkspaceGroup, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let group = Rc::new(TestExtWorkspaceGroupHandle {
            id: ev.workspace_group,
            manager: Rc::downgrade(_slf),
            removed: Cell::new(false),
            capabilities: Cell::new(0),
            outputs: Default::default(),
            workspaces: Default::default(),
        });
        self.client.set_synthetic_event_handler(group.id, &group);
        self.groups.borrow_mut().push(group);
        Ok(())
    }

    fn workspace(&self, ev: Workspace, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let workspace = Rc::new(TestExtWorkspaceHandle {
            id: ev.workspace,
            client: self.client.clone(),
            removed: Cell::new(false),
            state: Cell::new(0),
            capabilities: Cell::new(0),
            identifier: Default::default(),
            name: Default::default(),
            current_group: Cell::new(None),
        });
        self.client
            .set_synthetic_event_handler(workspace.id, &workspace);
        self.workspaces.borrow_mut().push(workspace);
        Ok(())
    }

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.done_count.set(self.done_count.get() + 1);
        Ok(())
    }

    fn finished(&self, _ev: Finished, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.finished.set(true);
        Ok(())
    }
}

impl TestClient {
    pub fn new_workspace_manager(&self) -> Rc<TestExtWorkspaceManager> {
        let id = self.client.bind(Singleton::ExtWorkspaceManagerV1);
        let manager = Rc::new(TestExtWorkspaceManager {
            id,
            client: self.client.clone(),
            finished: Cell::new(false),
            done_count: Cell::new(0),
            groups: Default::default(),
            workspaces: Default::default(),
        });
        self.client.set_synthetic_event_handler(id, &manager);
        manager
    }
}
