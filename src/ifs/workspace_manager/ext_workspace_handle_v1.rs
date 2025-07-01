use {
    crate::{
        client::{Client, ClientError},
        ifs::workspace_manager::{
            ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1,
            ext_workspace_manager_v1::{
                ExtWorkspaceManagerV1, WorkspaceChange, WorkspaceManagerId,
            },
            group_or_dangling,
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::{OutputNode, WorkspaceNode},
        utils::{clonecell::CloneCell, opt::Opt},
        wire::{ExtWorkspaceHandleV1Id, ext_workspace_handle_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

const STATE_ACTIVE: u32 = 1;
const STATE_URGENT: u32 = 2;
#[expect(dead_code)]
const STATE_HIDDEN: u32 = 4;

const CAP_ACTIVATE: u32 = 1;
#[expect(dead_code)]
const CAP_DEACTIVATE: u32 = 2;
#[expect(dead_code)]
const CAP_REMOVE: u32 = 4;
const CAP_ASSIGN: u32 = 8;

pub struct ExtWorkspaceHandleV1 {
    pub(super) id: ExtWorkspaceHandleV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub version: Version,
    pub(super) group: CloneCell<Rc<Opt<ExtWorkspaceGroupHandleV1>>>,
    pub(super) workspace: Rc<Opt<WorkspaceNode>>,
    pub(super) manager_id: WorkspaceManagerId,
    pub(super) manager: Rc<Opt<ExtWorkspaceManagerV1>>,
    pub(super) destroyed: Cell<bool>,
}

impl ExtWorkspaceHandleV1 {
    fn detach(&self) {
        if let Some(ws) = self.workspace.get() {
            ws.ext_workspaces.remove(&self.manager_id);
        }
    }

    pub(super) fn send_id(&self, id: &str) {
        self.client.event(Id {
            self_id: self.id,
            id,
        });
    }

    pub(super) fn send_name(&self, name: &str) {
        self.client.event(Name {
            self_id: self.id,
            name,
        });
    }

    #[expect(dead_code)]
    fn send_coordinates(&self, coordinates: &[u32]) {
        self.client.event(Coordinates {
            self_id: self.id,
            coordinates,
        });
    }

    pub(super) fn send_current_state(&self) {
        let Some(ws) = self.workspace.get() else {
            return;
        };
        let mut state = 0;
        let output = ws.output.get();
        if let Some(active) = output.workspace.get()
            && active.id == ws.id
        {
            state |= STATE_ACTIVE;
        }
        if ws.attention_requests.active() {
            state |= STATE_URGENT;
        }
        self.send_state(state);
    }

    fn send_state(&self, state: u32) {
        self.client.event(State {
            self_id: self.id,
            state,
        });
    }

    pub(super) fn send_capabilities(&self) {
        let capabilities = CAP_ACTIVATE | CAP_ASSIGN;
        self.client.event(Capabilities {
            self_id: self.id,
            capabilities,
        });
    }

    fn send_removed(&self) {
        self.client.event(Removed { self_id: self.id });
    }

    pub fn handle_destroyed(&self) {
        self.destroyed.set(true);
        if let Some(manager) = self.manager.get() {
            if let Some(group) = self.group.get().get() {
                group.send_workspace_leave(self);
            }
            self.group
                .set(self.client.state.workspace_managers.dangling_group.clone());
            self.send_state(0);
            self.send_removed();
            manager.schedule_done();
        }
    }

    pub fn handle_new_output(&self, output: &OutputNode) {
        let new = output.ext_workspace_groups.get(&self.manager_id);
        let new = group_or_dangling(&self.client, new.as_deref());
        let old = self.group.set(new.clone());
        if let Some(manager) = self.manager.get() {
            if let Some(old) = old.get() {
                old.send_workspace_leave(self);
            }
            if let Some(new) = new.get() {
                new.send_workspace_enter(self);
            }
            manager.schedule_done();
        }
    }

    pub fn handle_visibility_changed(&self) {
        if let Some(manager) = self.manager.get() {
            self.send_current_state();
            manager.schedule_done();
        }
    }

    pub fn handle_urgent_changed(&self) {
        self.handle_visibility_changed();
    }
}

object_base! {
    self = ExtWorkspaceHandleV1;
    version = self.version;
}

impl Object for ExtWorkspaceHandleV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ExtWorkspaceHandleV1);

impl ExtWorkspaceHandleV1RequestHandler for ExtWorkspaceHandleV1 {
    type Error = ExtWorkspaceHandleV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn activate(&self, _req: Activate, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }
        let Some(manager) = self.manager.get() else {
            return Ok(());
        };
        manager
            .pending
            .push(WorkspaceChange::ActivateWorkspace(self.workspace.clone()));
        Ok(())
    }

    fn deactivate(&self, _req: Deactivate, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn assign(&self, req: Assign, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }
        let group = self.client.lookup(req.workspace_group)?;
        let Some(manager) = self.manager.get() else {
            return Ok(());
        };
        manager.pending.push(WorkspaceChange::AssignWorkspace(
            self.workspace.clone(),
            group.output.clone(),
        ));
        Ok(())
    }

    fn remove(&self, _req: Remove, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ExtWorkspaceHandleV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtWorkspaceHandleV1Error, ClientError);
