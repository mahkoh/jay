use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            wl_output::{OutputGlobalOpt, WlOutput},
            workspace_manager::{
                ext_workspace_handle_v1::ExtWorkspaceHandleV1,
                ext_workspace_manager_v1::{
                    ExtWorkspaceManagerV1, WorkspaceChange, WorkspaceManagerId,
                },
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::opt::Opt,
        wire::{ExtWorkspaceGroupHandleV1Id, ext_workspace_group_handle_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtWorkspaceGroupHandleV1 {
    pub(super) id: ExtWorkspaceGroupHandleV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
    pub output: Rc<OutputGlobalOpt>,
    pub(super) manager_id: WorkspaceManagerId,
    pub(super) manager: Rc<Opt<ExtWorkspaceManagerV1>>,
    pub(super) opt: Rc<Opt<ExtWorkspaceGroupHandleV1>>,
}

const CAP_CREATE_WORKSPACE: u32 = 1;

impl ExtWorkspaceGroupHandleV1 {
    fn detach(&self) {
        self.opt.set(None);
        if let Some(node) = self.output.node() {
            node.ext_workspace_groups.remove(&self.manager_id);
        }
    }

    pub(super) fn send_capabilities(&self) {
        let capabilities = CAP_CREATE_WORKSPACE;
        self.client.event(Capabilities {
            self_id: self.id,
            capabilities,
        });
    }

    pub(super) fn send_output_enter(&self, output: &WlOutput) {
        self.client.event(OutputEnter {
            self_id: self.id,
            output: output.id,
        });
    }

    #[expect(dead_code)]
    fn send_output_leave(&self, output: &WlOutput) {
        self.client.event(OutputLeave {
            self_id: self.id,
            output: output.id,
        });
    }

    pub(super) fn send_workspace_enter(&self, workspace: &ExtWorkspaceHandleV1) {
        self.client.event(WorkspaceEnter {
            self_id: self.id,
            workspace: workspace.id,
        });
    }

    pub(super) fn send_workspace_leave(&self, workspace: &ExtWorkspaceHandleV1) {
        self.client.event(WorkspaceLeave {
            self_id: self.id,
            workspace: workspace.id,
        });
    }

    fn send_removed(&self) {
        self.client.event(Removed { self_id: self.id });
    }

    pub fn handle_destroyed(&self) {
        self.detach();
        if let Some(manager) = self.manager.get() {
            self.send_removed();
            manager.schedule_done();
        }
    }

    pub fn handle_new_output(&self, output: &WlOutput) {
        if let Some(manager) = self.manager.get() {
            self.send_output_enter(output);
            manager.schedule_done();
        }
    }
}

object_base! {
    self = ExtWorkspaceGroupHandleV1;
    version = self.version;
}

impl Object for ExtWorkspaceGroupHandleV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

dedicated_add_obj!(
    ExtWorkspaceGroupHandleV1,
    ExtWorkspaceGroupHandleV1Id,
    ext_workspace_groups
);

impl ExtWorkspaceGroupHandleV1RequestHandler for ExtWorkspaceGroupHandleV1 {
    type Error = ExtWorkspaceGroupHandleV1Error;

    fn create_workspace(
        &self,
        req: CreateWorkspace<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if self.opt.is_none() {
            return Ok(());
        }
        let Some(manager) = self.manager.get() else {
            return Ok(());
        };
        manager.pending.push(WorkspaceChange::CreateWorkspace(
            req.workspace.to_string(),
            self.output.clone(),
        ));
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(manager) = self.manager.get() {
            if let Some(node) = self.output.node() {
                let mut sent_any = false;
                for ws in node.workspaces.iter() {
                    if let Some(ws) = ws.ext_workspaces.get(&self.manager_id) {
                        self.send_workspace_leave(&ws);
                        sent_any = true;
                    }
                }
                if sent_any {
                    manager.schedule_done();
                }
            }
        }
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ExtWorkspaceGroupHandleV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtWorkspaceGroupHandleV1Error, ClientError);
