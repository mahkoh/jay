use {
    crate::{
        client::{Client, ClientError},
        ifs::jay_workspace::JayWorkspace,
        leaks::Tracker,
        object::Object,
        tree::WorkspaceNode,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
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
        });
        jw.send_linear_id(workspace);
        jw.send_name(workspace);
        jw.send_output(&workspace.output.get());
        jw.send_visible(workspace.visible.get());
        jw.send_done();
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), JayWorkspaceWatcherError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.remove_from_state();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn remove_from_state(&self) {
        self.client
            .state
            .workspace_watchers
            .remove(&(self.client.id, self.id));
    }
}

object_base! {
    JayWorkspaceWatcher;

    DESTROY => destroy,
}

impl Object for JayWorkspaceWatcher {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.remove_from_state();
    }
}

simple_add_obj!(JayWorkspaceWatcher);

#[derive(Debug, Error)]
pub enum JayWorkspaceWatcherError {
    #[error("Parsing failed")]
    MsgParserError(Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayWorkspaceWatcherError, MsgParserError);
efrom!(JayWorkspaceWatcherError, ClientError);
