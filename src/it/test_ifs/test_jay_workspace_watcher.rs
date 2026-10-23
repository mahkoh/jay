use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_jay_workspace::TestJayWorkspace;
use crate::wire::jay_workspace_watcher::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestJayWorkspaceWatcher {
    pub client: Rc<Client>,
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
}

synthetic_event_handler!(TestJayWorkspaceWatcher);

impl JayWorkspaceWatcherEventHandler for TestJayWorkspaceWatcher {
    type Error = TestErrorError;

    fn new(&self, ev: New, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let ws = Rc::new(TestJayWorkspace {
            id: ev.id,
            destroyed: Cell::new(false),
            linear_id: Cell::new(Some(ev.linear_id)),
            name: Default::default(),
            output: Default::default(),
            visible: Default::default(),
        });
        self.client.set_synthetic_event_handler(ws.id, &ws);
        self.workspaces.borrow_mut().push(ws);
        Ok(())
    }
}

impl TestClient {
    pub fn watch_workspaces(&self) -> Rc<TestJayWorkspaceWatcher> {
        let id = self.client.send_jay_compositor_watch_workspaces();
        let watcher = Rc::new(TestJayWorkspaceWatcher {
            client: self.client.clone(),
            workspaces: Default::default(),
        });
        self.client.set_synthetic_event_handler(id, &watcher);
        watcher
    }
}
