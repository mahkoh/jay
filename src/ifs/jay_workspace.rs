use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        tree::{OutputNode, WorkspaceNode},
        utils::clonecell::CloneCell,
        wire::{JayWorkspaceId, jay_workspace::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayWorkspace {
    pub id: JayWorkspaceId,
    pub client: Rc<Client>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub tracker: Tracker<Self>,
}

impl JayWorkspace {
    pub fn send_initial_properties(&self, workspace: &WorkspaceNode) {
        self.send_linear_id(workspace);
        self.send_name(workspace);
        self.send_output(&workspace.current.output.get());
        self.send_visible(workspace.current.visible.get());
        self.send_done();
    }

    pub fn send_linear_id(&self, ws: &WorkspaceNode) {
        self.client.event(LinearId {
            self_id: self.id,
            linear_id: ws.id.raw(),
        });
    }

    pub fn send_name(&self, ws: &WorkspaceNode) {
        self.client.event(Name {
            self_id: self.id,
            name: &ws.name,
        });
    }

    pub fn send_destroyed(&self) {
        self.client.event(Destroyed { self_id: self.id });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_output(&self, output: &OutputNode) {
        self.client.event(Output {
            self_id: self.id,
            global_name: output.global.name.raw(),
        });
    }

    pub fn send_visible(&self, visible: bool) {
        self.client.event(Visible {
            self_id: self.id,
            visible: visible as _,
        });
    }

    fn remove_from_node(&self) {
        if let Some(ws) = self.workspace.take() {
            ws.jay_workspaces.remove(&(self.client.id, self.id));
        }
    }
}

impl JayWorkspaceRequestHandler for JayWorkspace {
    type Error = JayWorkspaceError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.remove_from_node();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayWorkspace;
    version = Version(1);
}

impl Object for JayWorkspace {
    fn break_loops(self: Rc<Self>) {
        self.remove_from_node();
    }
}

dedicated_add_obj!(JayWorkspace, JayWorkspaceId, jay_workspaces);

#[derive(Debug, Error)]
pub enum JayWorkspaceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayWorkspaceError, ClientError);
