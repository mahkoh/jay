use {
    crate::{
        client::{CAP_WORKSPACE, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            wl_output::OutputGlobalOpt,
            workspace_manager::{
                ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1,
                ext_workspace_handle_v1::ExtWorkspaceHandleV1, group_or_dangling,
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::{OutputNode, WorkspaceNode, WsMoveConfig, move_ws_to_output},
        utils::{clonecell::CloneCell, opt::Opt, syncqueue::SyncQueue},
        wire::{ExtWorkspaceManagerV1Id, ext_workspace_manager_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

linear_ids!(WorkspaceManagerIds, WorkspaceManagerId, u64);

pub struct ExtWorkspaceManagerV1Global {
    pub name: GlobalName,
}

pub struct ExtWorkspaceManagerV1 {
    id: ExtWorkspaceManagerV1Id,
    pub(super) manager_id: WorkspaceManagerId,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
    pub(super) pending: SyncQueue<WorkspaceChange>,
    opt: Rc<Opt<ExtWorkspaceManagerV1>>,
    done_scheduled: Cell<bool>,
}

pub(super) enum WorkspaceChange {
    CreateWorkspace(String, Rc<OutputGlobalOpt>),
    ActivateWorkspace(Rc<Opt<WorkspaceNode>>),
    AssignWorkspace(Rc<Opt<WorkspaceNode>>, Rc<OutputGlobalOpt>),
}

impl ExtWorkspaceManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtWorkspaceManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ExtWorkspaceManagerV1Error> {
        let obj = Rc::new(ExtWorkspaceManagerV1 {
            id,
            manager_id: client.state.workspace_managers.ids.next(),
            client: client.clone(),
            tracker: Default::default(),
            version,
            pending: Default::default(),
            opt: Default::default(),
            done_scheduled: Cell::new(false),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        obj.opt.set(Some(obj.clone()));
        client
            .state
            .workspace_managers
            .managers
            .set(obj.manager_id, obj.clone());
        let dummy_output = client.state.dummy_output.get().unwrap();
        for ws in dummy_output.current_workspaces() {
            if !ws.is_dummy {
                obj.announce_workspace(&dummy_output, &ws);
            }
        }
        for output in client.state.root.outputs.lock().values() {
            obj.announce_output(output);
        }
        Ok(())
    }
}

impl ExtWorkspaceManagerV1 {
    pub(super) fn announce_output(&self, node: &OutputNode) {
        let id = match self.client.new_id() {
            Ok(id) => id,
            Err(e) => {
                self.client.error(e);
                return;
            }
        };
        let group = Rc::new(ExtWorkspaceGroupHandleV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            output: node.global.opt.clone(),
            manager_id: self.manager_id,
            manager: self.opt.clone(),
            opt: Default::default(),
        });
        track!(self.client, group);
        self.client.add_server_obj(&group);
        group.opt.set(Some(group.clone()));
        node.ext_workspace_groups
            .set(self.manager_id, group.clone());
        self.send_workspace_group(&group);
        group.send_capabilities();
        if let Some(bindings) = node.global.bindings.borrow().get(&self.client.id) {
            for wl_output in bindings.values() {
                group.send_output_enter(wl_output);
            }
        }
        for ws in node.current_workspaces() {
            if let Some(ws) = ws.ext_workspaces.get(&self.manager_id) {
                ws.handle_new_output(node);
            } else {
                self.announce_workspace(node, &ws);
            }
        }
        self.schedule_done();
    }

    pub(super) fn announce_workspace(&self, output: &OutputNode, workspace: &WorkspaceNode) {
        let id = match self.client.new_id() {
            Ok(id) => id,
            Err(e) => {
                self.client.error(e);
                return;
            }
        };
        let group = output.ext_workspace_groups.get(&self.manager_id);
        let ws = Rc::new(ExtWorkspaceHandleV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            group: CloneCell::new(group_or_dangling(&self.client, group.as_deref())),
            workspace: workspace.opt.clone(),
            manager_id: self.manager_id,
            manager: self.opt.clone(),
            destroyed: Cell::new(false),
        });
        track!(self.client, ws);
        self.client.add_server_obj(&ws);
        workspace.ext_workspaces.set(self.manager_id, ws.clone());
        self.send_workspace(&ws);
        ws.send_capabilities();
        ws.send_id(&workspace.name);
        ws.send_name(&workspace.name);
        ws.send_current_state();
        if let Some(group) = group {
            group.send_workspace_enter(&ws);
        }
        self.schedule_done();
    }

    fn send_workspace_group(&self, workspace_group: &ExtWorkspaceGroupHandleV1) {
        self.client.event(WorkspaceGroup {
            self_id: self.id,
            workspace_group: workspace_group.id,
        });
    }

    fn send_workspace(&self, workspace: &ExtWorkspaceHandleV1) {
        self.client.event(Workspace {
            self_id: self.id,
            workspace: workspace.id,
        });
    }

    pub(super) fn send_done(&self) {
        self.done_scheduled.set(false);
        self.client.event(Done { self_id: self.id });
    }

    fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id });
    }

    fn detach(&self) {
        self.opt.set(None);
        self.pending.clear();
        self.client
            .state
            .workspace_managers
            .managers
            .remove(&self.manager_id);
    }

    pub(super) fn schedule_done(&self) {
        if self.done_scheduled.replace(true) {
            return;
        }
        self.client
            .state
            .workspace_managers
            .queue
            .push(self.opt.clone());
    }
}

global_base!(
    ExtWorkspaceManagerV1Global,
    ExtWorkspaceManagerV1,
    ExtWorkspaceManagerV1Error
);

impl Global for ExtWorkspaceManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_WORKSPACE
    }
}

simple_add_global!(ExtWorkspaceManagerV1Global);

object_base! {
    self = ExtWorkspaceManagerV1;
    version = self.version;
}

impl Object for ExtWorkspaceManagerV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(ExtWorkspaceManagerV1);

impl ExtWorkspaceManagerV1RequestHandler for ExtWorkspaceManagerV1 {
    type Error = ExtWorkspaceManagerV1Error;

    fn commit(&self, _req: Commit, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tt = &self.client.state.tree_transaction();
        while let Some(change) = self.pending.pop() {
            match change {
                WorkspaceChange::ActivateWorkspace(w) => {
                    let Some(ws) = w.get() else {
                        continue;
                    };
                    let output = ws.current.output.get();
                    let seat = self.client.state.seat_queue.last().as_deref().cloned();
                    self.client
                        .state
                        .show_workspace2(tt, seat.as_ref(), &output, &ws);
                }
                WorkspaceChange::AssignWorkspace(w, o) => {
                    let Some(ws) = w.get() else {
                        continue;
                    };
                    let Some(o) = o.node() else {
                        continue;
                    };
                    let config = WsMoveConfig {
                        make_visible_always: false,
                        make_visible_if_empty: true,
                        source_is_destroyed: false,
                        before: None,
                    };
                    move_ws_to_output(tt, &ws, &o, config);
                    ws.desired_output.set(o.global.output_id.clone());
                    self.client.state.tree_changed();
                }
                WorkspaceChange::CreateWorkspace(name, output) => {
                    if self.client.state.workspaces.contains(&name) {
                        return Ok(());
                    }
                    let Some(output) = output.node() else {
                        return Ok(());
                    };
                    output.create_workspace(tt, &name);
                }
            }
        }
        Ok(())
    }

    fn stop(&self, _req: Stop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.send_finished();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ExtWorkspaceManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtWorkspaceManagerV1Error, ClientError);
