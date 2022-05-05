use {
    crate::{
        client::Client,
        ifs::wl_seat::{collect_kb_foci, collect_kb_foci2, NodeSeatState, SeatId},
        rect::Rect,
        state::State,
        tree::{ContainingNode, Node, OutputNode, PlaceholderNode, WorkspaceNode},
        utils::{clonecell::CloneCell, numcell::NumCell, smallmap::SmallMap},
    },
    jay_config::Direction,
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
};

tree_id!(ToplevelNodeId);

pub trait ToplevelNode: Node {
    fn tl_as_node(&self) -> &dyn Node;
    fn tl_into_node(self: Rc<Self>) -> Rc<dyn Node>;
    fn tl_into_dyn(self: Rc<Self>) -> Rc<dyn ToplevelNode>;

    fn tl_data(&self) -> &ToplevelData;

    fn tl_default_focus_child(&self) -> Option<Rc<dyn Node>> {
        None
    }

    fn tl_accepts_keyboard_focus(&self) -> bool {
        true
    }

    fn tl_set_active(&self, active: bool) {
        let _ = active;
    }

    fn tl_on_activate(&self) {
        // nothing
    }

    fn tl_surface_active_changed(&self, active: bool) {
        let data = self.tl_data();
        if active {
            if data.active_surfaces.fetch_add(1) == 0 {
                self.tl_set_active(true);
                if let Some(parent) = data.parent.get() {
                    parent.node_child_active_changed(self.tl_as_node(), true, 1);
                }
            }
        } else {
            if data.active_surfaces.fetch_sub(1) == 1 {
                self.tl_set_active(false);
                if let Some(parent) = data.parent.get() {
                    parent.node_child_active_changed(self.tl_as_node(), false, 1);
                }
            }
        }
    }

    fn tl_focus_child(&self, seat: SeatId) -> Option<Rc<dyn Node>> {
        self.tl_data()
            .focus_node
            .get(&seat)
            .or_else(|| self.tl_default_focus_child())
    }

    fn tl_set_fullscreen(self: Rc<Self>, fullscreen: bool) {
        let data = self.tl_data();
        if fullscreen {
            if let Some(ws) = data.workspace.get() {
                data.set_fullscreen2(&data.state, self.clone().tl_into_dyn(), &ws);
            }
        } else {
            data.unset_fullscreen(&data.state, self.clone().tl_into_dyn());
        }
    }

    fn tl_title_changed(&self) {
        let data = self.tl_data();
        let title = data.title.borrow_mut();
        if let Some(parent) = data.parent.get() {
            parent.node_child_title_changed(self.tl_as_node(), &title);
        }
        if let Some(data) = data.fullscrceen_data.borrow_mut().deref() {
            *data.placeholder.tl_data().title.borrow_mut() = title.clone();
            data.placeholder.tl_title_changed();
        }
    }

    fn tl_set_parent(&self, parent: Rc<dyn ContainingNode>) {
        let data = self.tl_data();
        data.parent.set(Some(parent.clone()));
        data.is_floating.set(parent.node_is_float());
        self.tl_extents_changed();
        self.tl_title_changed();
        self.tl_active_changed();
        self.tl_after_parent_set(parent);
    }

    fn tl_after_parent_set(&self, parent: Rc<dyn ContainingNode>) {
        let _ = parent;
    }

    fn tl_active_changed(&self) {
        let data = self.tl_data();
        let parent = match data.parent.get() {
            Some(p) => p,
            _ => return,
        };
        let node = self.tl_as_node();
        if data.active.get() || data.active_surfaces.get() > 0 {
            parent.clone().node_child_active_changed(node, true, 1);
        }
    }

    fn tl_extents_changed(&self) {
        let data = self.tl_data();
        let parent = match data.parent.get() {
            Some(p) => p,
            _ => return,
        };
        let node = self.tl_as_node();
        let pos = data.pos.get();
        parent.node_child_size_changed(node, pos.width(), pos.height());
        data.state.tree_changed();
    }

    fn tl_set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        let data = self.tl_data();
        data.workspace.set(Some(ws.clone()));
    }

    fn tl_change_extents(self: Rc<Self>, rect: &Rect) {
        let _ = rect;
    }

    fn tl_close(self: Rc<Self>) {
        // nothing
    }

    fn tl_set_visible(&self, visible: bool);
    fn tl_destroy(&self);

    fn tl_last_active_child(self: Rc<Self>) -> Rc<dyn ToplevelNode> {
        self.tl_into_dyn()
    }
}

pub struct FullscreenedData {
    pub placeholder: Rc<PlaceholderNode>,
    pub workspace: Rc<WorkspaceNode>,
}

pub struct ToplevelData {
    pub active: Cell<bool>,
    pub client: Option<Rc<Client>>,
    pub state: Rc<State>,
    pub active_surfaces: NumCell<u32>,
    pub focus_node: SmallMap<SeatId, Rc<dyn Node>, 1>,
    pub visible: Cell<bool>,
    pub is_floating: Cell<bool>,
    pub float_width: Cell<i32>,
    pub float_height: Cell<i32>,
    pub is_fullscreen: Cell<bool>,
    pub fullscrceen_data: RefCell<Option<FullscreenedData>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub title: RefCell<String>,
    pub parent: CloneCell<Option<Rc<dyn ContainingNode>>>,
    pub pos: Cell<Rect>,
    pub seat_state: NodeSeatState,
}

impl ToplevelData {
    pub fn new(state: &Rc<State>, title: String, client: Option<Rc<Client>>) -> Self {
        Self {
            active: Cell::new(false),
            client,
            state: state.clone(),
            active_surfaces: Default::default(),
            focus_node: Default::default(),
            visible: Cell::new(false),
            is_floating: Default::default(),
            float_width: Default::default(),
            float_height: Default::default(),
            is_fullscreen: Default::default(),
            fullscrceen_data: Default::default(),
            workspace: Default::default(),
            title: RefCell::new(title),
            parent: Default::default(),
            pos: Default::default(),
            seat_state: Default::default(),
        }
    }

    pub fn float_size(&self, ws: &WorkspaceNode) -> (i32, i32) {
        let output = ws.output.get().global.pos.get();
        let mut width = self.float_width.get();
        let mut height = self.float_height.get();
        if width == 0 {
            width = output.width() / 2;
        }
        if height == 0 {
            height = output.height() / 2;
        }
        (width, height)
    }

    pub fn destroy_node(&self, node: &dyn Node) {
        if let Some(fd) = self.fullscrceen_data.borrow_mut().take() {
            fd.placeholder.tl_destroy();
        }
        if let Some(parent) = self.parent.take() {
            parent.cnode_remove_child(node);
        }
        self.workspace.take();
        self.seat_state.destroy_node(node);
        self.focus_node.clear();
    }

    pub fn set_fullscreen(
        &self,
        state: &Rc<State>,
        node: Rc<dyn ToplevelNode>,
        output: &Rc<OutputNode>,
    ) {
        self.set_fullscreen2(state, node, &output.ensure_workspace());
    }

    pub fn set_fullscreen2(
        &self,
        state: &Rc<State>,
        node: Rc<dyn ToplevelNode>,
        ws: &Rc<WorkspaceNode>,
    ) {
        if ws.fullscreen.get().is_some() {
            log::info!("Cannot fullscreen a node on a workspace that already has a fullscreen node attached");
            return;
        }
        if node.node_is_placeholder() {
            log::info!("Cannot fullscreen a placeholder node");
            return;
        }
        let mut data = self.fullscrceen_data.borrow_mut();
        if data.is_some() {
            log::info!("Cannot fullscreen a node that is already fullscreen");
            return;
        }
        let parent = match node.tl_data().parent.get() {
            None => {
                log::warn!("Cannot fullscreen a node without a parent");
                return;
            }
            Some(p) => p,
        };
        let placeholder = Rc::new(PlaceholderNode::new_for(state, node.clone()));
        parent.cnode_replace_child(node.tl_as_node(), placeholder.clone());
        let mut kb_foci = Default::default();
        if ws.visible.get() {
            if let Some(container) = ws.container.get() {
                kb_foci = collect_kb_foci(container.clone());
                container.tl_set_visible(false);
            }
            for stacked in ws.stacked.iter() {
                collect_kb_foci2(stacked.deref().clone().stacked_into_node(), &mut kb_foci);
                stacked.stacked_set_visible(false);
            }
        }
        *data = Some(FullscreenedData {
            placeholder,
            workspace: ws.clone(),
        });
        drop(data);
        self.is_fullscreen.set(true);
        ws.fullscreen.set(Some(node.clone()));
        node.tl_set_parent(ws.clone());
        node.clone().tl_set_workspace(&ws);
        node.clone()
            .tl_change_extents(&ws.output.get().global.pos.get());
        for seat in kb_foci {
            node.clone()
                .tl_into_node()
                .node_do_focus(&seat, Direction::Unspecified);
        }
    }

    pub fn unset_fullscreen(&self, state: &Rc<State>, node: Rc<dyn ToplevelNode>) {
        if !self.is_fullscreen.get() {
            log::warn!("Cannot unset fullscreen on a node that is not fullscreen");
            return;
        }
        let fd = match self.fullscrceen_data.borrow_mut().take() {
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
            Some(f) if f.tl_as_node().node_id() != node.tl_as_node().node_id() => {
                log::error!("Node is supposed to be fullscreened on a workspace but the workspace has a different node attached.");
                return;
            }
            _ => {}
        }
        fd.workspace.fullscreen.take();
        if node.node_visible() {
            fd.workspace.set_visible(true);
        }
        if fd.placeholder.is_destroyed() {
            state.map_tiled(node);
            return;
        }
        let parent = fd.placeholder.tl_data().parent.get().unwrap();
        parent.cnode_replace_child(fd.placeholder.deref(), node.clone());
        if node.tl_as_node().node_visible() {
            let kb_foci = collect_kb_foci(fd.placeholder.clone());
            for seat in kb_foci {
                node.clone()
                    .tl_into_node()
                    .node_do_focus(&seat, Direction::Unspecified);
            }
        }
        fd.placeholder
            .node_seat_state()
            .destroy_node(fd.placeholder.deref());
    }

    pub fn set_visible(&self, node: &dyn Node, visible: bool) {
        self.visible.set(visible);
        self.seat_state.set_visible(node, visible)
    }
}
