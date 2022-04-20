use {
    crate::tree::{Node, PlaceholderNode, SizedNode, WorkspaceNode},
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
};
use jay_config::Direction;
use crate::ifs::wl_seat::collect_kb_foci;
use crate::state::State;
use crate::tree::OutputNode;

pub trait SizedFullscreenNode: SizedNode {
    fn on_set_fullscreen(&self, workspace: &Rc<WorkspaceNode>);
    fn on_unset_fullscreen(&self);
    fn title(&self) -> String;

    fn as_node(&self) -> &dyn Node {
        self
    }

    fn into_node(self: Rc<Self>) -> Rc<dyn Node> {
        self
    }
}

pub trait FullscreenNode {
    fn on_set_fullscreen(&self, workspace: &Rc<WorkspaceNode>);
    fn on_unset_fullscreen(&self);
    fn as_node(&self) -> &dyn Node;
    fn into_node(self: Rc<Self>) -> Rc<dyn Node>;
    fn title(&self) -> String;
}

impl<T: SizedFullscreenNode> FullscreenNode for T {
    fn on_set_fullscreen(&self, workspace: &Rc<WorkspaceNode>) {
        <Self as SizedFullscreenNode>::on_set_fullscreen(self, workspace)
    }

    fn on_unset_fullscreen(&self) {
        <Self as SizedFullscreenNode>::on_unset_fullscreen(self)
    }

    fn as_node(&self) -> &dyn Node {
        <Self as SizedFullscreenNode>::as_node(self)
    }

    fn into_node(self: Rc<Self>) -> Rc<dyn Node> {
        <Self as SizedFullscreenNode>::into_node(self)
    }

    fn title(&self) -> String {
        <Self as SizedFullscreenNode>::title(self)
    }
}

pub struct FullscreenedData {
    pub placeholder: Rc<PlaceholderNode>,
    pub workspace: Rc<WorkspaceNode>,
}

#[derive(Default)]
pub struct FullscreenData {
    pub is_fullscreen: Cell<bool>,
    pub data: RefCell<Option<FullscreenedData>>,
}

impl FullscreenData {
    pub fn set_title(&self, title: &str) {
        let data = self.data.borrow_mut();
        if let Some(data) = data.deref() {
            data.placeholder.set_title(title);
        }
    }
}

impl FullscreenData {
    pub fn set_fullscreen(&self, state: &Rc<State>, node: Rc<dyn FullscreenNode>, output: &Rc<OutputNode>) {
        let ws = output.ensure_workspace();
        if ws.fullscreen.get().is_some() {
            log::info!("Cannot fullscreen a node on a workspace that already has a fullscreen node attached");
            return;
        }
        let mut data = self.data.borrow_mut();
        if data.is_some() {
            log::info!("Cannot fullscreen a node that is already fullscreen");
            return;
        }
        let parent = match node.as_node().node_parent() {
            None => {
                log::warn!("Cannot fullscreen a node without a parent");
                return;
            }
            Some(p) => p,
        };
        let placeholder = Rc::new(PlaceholderNode::new_for(state, node.clone()));
        parent.node_replace_child(node.as_node(), placeholder.clone());
        let mut kb_foci = Default::default();
        if let Some(container) = ws.container.get() {
            kb_foci = collect_kb_foci(container.clone());
            container.set_visible(false);
        }
        *data = Some(FullscreenedData {
            placeholder,
            workspace: ws.clone(),
        });
        self.is_fullscreen.set(true);
        ws.fullscreen.set(Some(node.clone()));
        node.clone().into_node().node_set_parent(ws.clone());
        node.clone().into_node().node_set_workspace(&ws);
        node.clone()
            .into_node()
            .node_change_extents(&output.global.pos.get());
        for seat in kb_foci {
            node.clone()
                .into_node()
                .node_do_focus(&seat, Direction::Unspecified);
        }
        node.on_set_fullscreen(&ws);
    }

    pub fn unset_fullscreen(&self, state: &Rc<State>, node: Rc<dyn FullscreenNode>) {
        if !self.is_fullscreen.get() {
            log::warn!("Cannot unset fullscreen on a node that is not fullscreen");
            return;
        }
        let fd = match self.data.borrow_mut().take() {
            Some(fd) => fd,
            _ => {
                log::error!("is_fullscreen = true but data is None");
                return;
            }
        };
        self.is_fullscreen.set(false);
        match fd.workspace.fullscreen.get() {
            None => {
                log::error!("Node is supposed to be fullscreened on a workspace but workspace has not fullscreen node.");
                return;
            }
            Some(f) if f.as_node().node_id() != node.as_node().node_id() => {
                log::error!("Node is supposed to be fullscreened on a workspace but the workspace has a different node attached.");
                return;
            }
            _ => {}
        }
        fd.workspace.fullscreen.take();
        if let Some(container) = fd.workspace.container.get() {
            container.set_visible(true);
        }
        if fd.placeholder.is_destroyed() {
            state.map_tiled(node.into_node());
            return;
        }
        let parent = fd.placeholder.parent().unwrap();
        parent.node_replace_child(fd.placeholder.deref(), node.clone().into_node());
        if node.as_node().node_visible() {
            let kb_foci = collect_kb_foci(fd.placeholder.clone());
            for seat in kb_foci {
                node.clone()
                    .into_node()
                    .node_do_focus(&seat, Direction::Unspecified);
            }
        }
        fd.placeholder
            .node_seat_state()
            .destroy_node(fd.placeholder.deref());
        node.on_unset_fullscreen();
    }

    pub fn destroy_node(&self) {
        if let Some(fd) = self.data.borrow_mut().take() {
            fd.placeholder.destroy_node(true);
        }
    }
}
