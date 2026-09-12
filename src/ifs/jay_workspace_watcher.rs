use crate::client::Client;
use crate::ifs::jay_workspace::JayWorkspace;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::tree::WorkspaceNode;
use crate::utils::clonecell::CloneCell;
use crate::wire::JayWorkspaceWatcherId;
use crate::wire::jay_workspace_watcher::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct JayWorkspaceWatcher {
    pub id: JayWorkspaceWatcherId,
    pub version: Version,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JayWorkspaceWatcher {
    pub fn send_workspace(&self, workspace: &Rc<WorkspaceNode>) {
        let jw = Rc::new(JayWorkspace {
            id: self.client.new_id(self),
            version: self.version,
            client: self.client.clone(),
            workspace: CloneCell::new(Some(workspace.clone())),
            tracker: Default::default(),
        });
        track!(self.client, jw);
        self.client.add_server_obj(&jw);
        workspace
            .jay_workspaces
            .set((self.client.id, jw.id), jw.clone());
        self.client.event(New {
            self_id: self.id,
            id: jw.id,
            linear_id: workspace.id.raw(),
        });
        jw.send_initial_properties(workspace);
    }

    fn remove_from_state(&self) {
        self.client
            .state
            .workspace_watchers
            .remove(&(self.client.id, self.id));
    }
}

impl JayWorkspaceWatcherRequestHandler for JayWorkspaceWatcher {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.remove_from_state();
        self.client.remove_obj(self);
        Ok(())
    }
}

impl BreakLoops for JayWorkspaceWatcher {
    fn break_loops(self: Rc<Self>) {
        self.remove_from_state();
    }
}
