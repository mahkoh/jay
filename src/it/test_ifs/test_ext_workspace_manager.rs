use crate::client::Client;
use crate::globals::Singleton;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_ext_workspace_group_handle::TestExtWorkspaceGroupHandle;
use crate::it::test_ifs::test_ext_workspace_handle::TestExtWorkspaceHandle;
use crate::wire::ExtWorkspaceManagerV1Id;
use crate::wire::ext_workspace_manager_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestExtWorkspaceManager {
    #[expect(unused)]
    pub id: ExtWorkspaceManagerV1Id,
    pub client: Rc<Client>,
    pub groups: RefCell<Vec<Rc<TestExtWorkspaceGroupHandle>>>,
    pub workspaces: RefCell<Vec<Rc<TestExtWorkspaceHandle>>>,
    pub missing_coordinates_at_done: Cell<bool>,
}

impl TestClient {
    pub fn new_workspace_manager(&self) -> Rc<TestExtWorkspaceManager> {
        let id = self.client.bind(Singleton::ExtWorkspaceManagerV1);
        let slf = Rc::new(TestExtWorkspaceManager {
            id,
            client: self.client.clone(),
            groups: Default::default(),
            workspaces: Default::default(),
            missing_coordinates_at_done: Default::default(),
        });
        self.client.set_synthetic_event_handler(id, &slf);
        slf
    }
}

impl TestExtWorkspaceManager {
    pub fn workspace(&self, name: &str) -> Option<Rc<TestExtWorkspaceHandle>> {
        let workspaces = self.workspaces.borrow();
        workspaces
            .iter()
            .find(|w| !w.removed.get() && w.name.borrow().as_str() == name)
            .cloned()
    }
}

synthetic_event_handler!(TestExtWorkspaceManager);

impl ExtWorkspaceManagerV1EventHandler for TestExtWorkspaceManager {
    type Error = TestErrorError;

    fn workspace_group(&self, ev: WorkspaceGroup, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let group = Rc::new(TestExtWorkspaceGroupHandle {
            id: ev.workspace_group,
        });
        self.client
            .set_synthetic_event_handler(ev.workspace_group, &group);
        self.groups.borrow_mut().push(group);
        Ok(())
    }

    fn workspace(&self, ev: Workspace, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let ws = Rc::new(TestExtWorkspaceHandle {
            id: ev.workspace,
            name: Default::default(),
            coordinates: Default::default(),
            removed: Default::default(),
        });
        self.client.set_synthetic_event_handler(ev.workspace, &ws);
        self.workspaces.borrow_mut().push(ws);
        Ok(())
    }

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let missing = self
            .workspaces
            .borrow()
            .iter()
            .any(|ws| !ws.removed.get() && ws.coordinates.borrow().is_empty());
        if missing {
            self.missing_coordinates_at_done.set(true);
        }
        Ok(())
    }

    fn finished(&self, _ev: Finished, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}
