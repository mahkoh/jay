use {
    crate::{
        client::{Client, ClientError},
        ifs::jay_workspace::JayWorkspace,
        leaks::Tracker,
        object::{Object, Version},
        tree::WorkspaceNode,
        utils::clonecell::CloneCell,
        wire::{jay_workspace_watcher::*, JayWorkspaceWatcherId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayWorkspaceWatcher {
    pub id: JayWorkspaceWatcherId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JayWorkspaceWatcher {
    pub fn send_workspace(&self, workspace: &Rc<WorkspaceNode>) -> Result<(), ClientError> {
        let jw = Rc::new(JayWorkspace {
            id: self.client.new_id()?,
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
        Ok(())
    }

    fn remove_from_state(&self) {
        self.client
            .state
            .workspace_watchers
            .remove(&(self.client.id, self.id));
    }
}

impl JayWorkspaceWatcherRequestHandler for JayWorkspaceWatcher {
    type Error = JayWorkspaceWatcherError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.remove_from_state();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayWorkspaceWatcher;
    version = Version(1);
}

impl Object for JayWorkspaceWatcher {
    fn break_loops(&self) {
        self.remove_from_state();
    }
}

simple_add_obj!(JayWorkspaceWatcher);

#[derive(Debug, Error)]
pub enum JayWorkspaceWatcherError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayWorkspaceWatcherError, ClientError);
