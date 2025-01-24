use {
    crate::{
        client::Client,
        ifs::workspace_manager::{
            ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1,
            ext_workspace_manager_v1::{
                ExtWorkspaceManagerV1, WorkspaceManagerId, WorkspaceManagerIds,
            },
        },
        state::State,
        tree::{OutputNode, WorkspaceNode},
        utils::{copyhashmap::CopyHashMap, opt::Opt, queue::AsyncQueue},
    },
    std::rc::Rc,
};

pub mod ext_workspace_group_handle_v1;
pub mod ext_workspace_handle_v1;
pub mod ext_workspace_manager_v1;

#[derive(Default)]
pub struct WorkspaceManagerState {
    queue: AsyncQueue<Rc<Opt<ExtWorkspaceManagerV1>>>,
    dangling_group: Rc<Opt<ExtWorkspaceGroupHandleV1>>,
    ids: WorkspaceManagerIds,
    managers: CopyHashMap<WorkspaceManagerId, Rc<ExtWorkspaceManagerV1>>,
}

impl WorkspaceManagerState {
    pub fn clear(&self) {
        self.managers.clear();
        self.queue.clear();
    }

    pub fn announce_output(&self, on: &OutputNode) {
        for manager in self.managers.lock().values() {
            manager.announce_output(on);
        }
    }

    pub fn announce_workspace(&self, output: &OutputNode, ws: &WorkspaceNode) {
        for manager in self.managers.lock().values() {
            manager.announce_workspace(output, ws);
        }
    }
}

pub async fn workspace_manager_done(state: Rc<State>) {
    loop {
        let manager = state.workspace_managers.queue.pop().await;
        if let Some(manager) = manager.get() {
            manager.send_done();
        }
    }
}

fn group_or_dangling(
    client: &Client,
    group: Option<&ExtWorkspaceGroupHandleV1>,
) -> Rc<Opt<ExtWorkspaceGroupHandleV1>> {
    group
        .map(|g| g.opt.clone())
        .unwrap_or_else(|| client.state.workspace_managers.dangling_group.clone())
}
