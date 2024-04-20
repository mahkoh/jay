use {
    crate::{
        client::{Client, ClientError},
        ifs::{jay_workspace::JayWorkspace, wl_seat::WorkspaceSelector},
        leaks::Tracker,
        object::{Object, Version},
        tree::WorkspaceNode,
        utils::clonecell::CloneCell,
        wire::{jay_select_workspace::*, JaySelectWorkspaceId, JayWorkspaceId},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JaySelectWorkspace {
    pub id: JaySelectWorkspaceId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub destroyed: Cell<bool>,
}

pub struct JayWorkspaceSelector {
    pub ws: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub jsw: Rc<JaySelectWorkspace>,
}

impl WorkspaceSelector for JayWorkspaceSelector {
    fn set(&self, ws: Rc<WorkspaceNode>) {
        self.ws.set(Some(ws));
    }
}

impl Drop for JayWorkspaceSelector {
    fn drop(&mut self) {
        if self.jsw.destroyed.get() {
            return;
        }
        match self.ws.take() {
            None => {
                self.jsw.send_cancelled();
            }
            Some(ws) => {
                let id = match self.jsw.client.new_id() {
                    Ok(id) => id,
                    Err(e) => {
                        self.jsw.client.error(e);
                        return;
                    }
                };
                let jw = Rc::new(JayWorkspace {
                    id,
                    client: self.jsw.client.clone(),
                    workspace: CloneCell::new(Some(ws.clone())),
                    tracker: Default::default(),
                });
                track!(self.jsw.client, jw);
                self.jsw.client.add_server_obj(&jw);
                self.jsw
                    .send_selected(ws.output.get().global.name.raw(), id);
                ws.jay_workspaces
                    .set((self.jsw.client.id, jw.id), jw.clone());
                jw.send_initial_properties(&ws);
            }
        };
        let _ = self.jsw.client.remove_obj(&*self.jsw);
    }
}

impl JaySelectWorkspace {
    fn send_cancelled(&self) {
        self.client.event(Cancelled { self_id: self.id });
    }

    fn send_selected(&self, output: u32, id: JayWorkspaceId) {
        self.client.event(Selected {
            self_id: self.id,
            output,
            id,
        });
    }
}

impl JaySelectWorkspaceRequestHandler for JaySelectWorkspace {
    type Error = JaySelectWorkspaceError;
}

object_base! {
    self = JaySelectWorkspace;
    version = Version(1);
}

impl Object for JaySelectWorkspace {
    fn break_loops(&self) {
        self.destroyed.set(true);
    }
}

simple_add_obj!(JaySelectWorkspace);

#[derive(Debug, Error)]
pub enum JaySelectWorkspaceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JaySelectWorkspaceError, ClientError);
