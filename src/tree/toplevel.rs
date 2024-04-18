use {
    crate::{
        client::{Client, ClientId},
        ifs::{
            ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
            ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
            wl_seat::{collect_kb_foci, collect_kb_foci2, NodeSeatState, SeatId},
            wl_surface::WlSurface,
        },
        rect::Rect,
        state::State,
        tree::{ContainingNode, Direction, Node, OutputNode, PlaceholderNode, WorkspaceNode},
        utils::{
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            numcell::NumCell,
            smallmap::SmallMap,
            threshold_counter::ThresholdCounter,
            toplevel_identifier::{toplevel_identifier, ToplevelIdentifier},
        },
        wire::ExtForeignToplevelHandleV1Id,
    },
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
};

tree_id!(ToplevelNodeId);

pub trait ToplevelNode: ToplevelNodeBase {
    fn tl_as_node(&self) -> &dyn Node;
    fn tl_into_node(self: Rc<Self>) -> Rc<dyn Node>;
    fn tl_into_dyn(self: Rc<Self>) -> Rc<dyn ToplevelNode>;
    fn tl_surface_active_changed(&self, active: bool);
    fn tl_set_fullscreen(self: Rc<Self>, fullscreen: bool);
    fn tl_title_changed(&self);
    fn tl_set_parent(&self, parent: Rc<dyn ContainingNode>);
    fn tl_extents_changed(&self);
    fn tl_set_workspace(&self, ws: &Rc<WorkspaceNode>);
    fn tl_change_extents(self: Rc<Self>, rect: &Rect);
    fn tl_set_visible(&self, visible: bool);
    fn tl_destroy(&self);
}

impl<T: ToplevelNodeBase> ToplevelNode for T {
    fn tl_as_node(&self) -> &dyn Node {
        self
    }

    fn tl_into_node(self: Rc<Self>) -> Rc<dyn Node> {
        self
    }

    fn tl_into_dyn(self: Rc<Self>) -> Rc<dyn ToplevelNode> {
        self
    }

    fn tl_surface_active_changed(&self, active: bool) {
        let data = self.tl_data();
        data.update_active(self, || {
            data.active_surfaces.adj(active);
        });
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
            parent.node_child_title_changed(self, &title);
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
        self.tl_set_workspace(&parent.cnode_workspace());
    }

    fn tl_extents_changed(&self) {
        let data = self.tl_data();
        if let Some(parent) = data.parent.get() {
            let pos = data.pos.get();
            parent.node_child_size_changed(self, pos.width(), pos.height());
            data.state.tree_changed();
        }
    }

    fn tl_set_workspace(&self, ws: &Rc<WorkspaceNode>) {
        let data = self.tl_data();
        data.workspace.set(Some(ws.clone()));
        self.tl_set_workspace_ext(ws);
    }

    fn tl_change_extents(self: Rc<Self>, rect: &Rect) {
        let data = self.tl_data();
        if data.is_floating.get() {
            data.float_width.set(rect.width());
            data.float_height.set(rect.height());
        }
        self.tl_change_extents_impl(rect)
    }

    fn tl_set_visible(&self, visible: bool) {
        self.tl_set_visible_impl(visible);
        self.tl_data().set_visible(self, visible);
    }

    fn tl_destroy(&self) {
        self.tl_data().destroy_node(self);
        self.tl_destroy_impl();
    }
}

pub trait ToplevelNodeBase: Node {
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

    fn tl_focus_child(&self, seat: SeatId) -> Option<Rc<dyn Node>> {
        self.tl_data()
            .focus_node
            .get(&seat)
            .or_else(|| self.tl_default_focus_child())
    }

    fn tl_set_workspace_ext(&self, ws: &Rc<WorkspaceNode>) {
        let _ = ws;
    }

    fn tl_change_extents_impl(self: Rc<Self>, rect: &Rect);

    fn tl_close(self: Rc<Self>);

    fn tl_set_visible_impl(&self, visible: bool);
    fn tl_destroy_impl(&self);

    fn tl_last_active_child(self: Rc<Self>) -> Rc<dyn ToplevelNode>;

    fn tl_scanout_surface(&self) -> Option<Rc<WlSurface>> {
        None
    }
    fn tl_restack_popups(&self) {
        // nothing
    }

    fn tl_admits_children(&self) -> bool;
}

pub struct FullscreenedData {
    pub placeholder: Rc<PlaceholderNode>,
    pub workspace: Rc<WorkspaceNode>,
}

pub struct ToplevelData {
    pub self_active: Cell<bool>,
    pub client: Option<Rc<Client>>,
    pub state: Rc<State>,
    pub active_surfaces: ThresholdCounter,
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
    pub wants_attention: Cell<bool>,
    pub requested_attention: Cell<bool>,
    pub app_id: RefCell<String>,
    pub identifier: Cell<ToplevelIdentifier>,
    pub handles:
        CopyHashMap<(ClientId, ExtForeignToplevelHandleV1Id), Rc<ExtForeignToplevelHandleV1>>,
    pub render_highlight: NumCell<u32>,
}

impl ToplevelData {
    pub fn new(state: &Rc<State>, title: String, client: Option<Rc<Client>>) -> Self {
        Self {
            self_active: Cell::new(false),
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
            wants_attention: Cell::new(false),
            requested_attention: Cell::new(false),
            app_id: Default::default(),
            identifier: Cell::new(toplevel_identifier()),
            handles: Default::default(),
            render_highlight: Default::default(),
        }
    }

    pub fn active(&self) -> bool {
        self.active_surfaces.active() || self.self_active.get()
    }

    fn update_active<T: ToplevelNode, F: FnOnce()>(&self, tl: &T, f: F) {
        let active_old = self.active();
        f();
        let active_new = self.active();
        if active_old != active_new {
            tl.tl_set_active(active_new);
            if let Some(parent) = self.parent.get() {
                parent.node_child_active_changed(tl.tl_as_node(), active_new, 1);
            }
        }
    }

    pub fn update_self_active<T: ToplevelNode>(&self, node: &T, active: bool) {
        self.update_active(node, || self.self_active.set(active));
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
        self.identifier.set(toplevel_identifier());
        {
            let mut handles = self.handles.lock();
            for (_, handle) in handles.drain() {
                handle.send_closed();
            }
        }
        self.detach_node(node);
    }

    pub fn detach_node(&self, node: &dyn Node) {
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

    pub fn broadcast(&self, toplevel: Rc<dyn ToplevelNode>) {
        let id = self.identifier.get().to_string();
        let title = self.title.borrow();
        let app_id = self.app_id.borrow();
        for list in self.state.toplevel_lists.lock().values() {
            self.send_once(&toplevel, list, &id, &title, &app_id);
        }
    }

    pub fn send(&self, toplevel: Rc<dyn ToplevelNode>, list: &ExtForeignToplevelListV1) {
        let id = self.identifier.get().to_string();
        let title = self.title.borrow();
        let app_id = self.app_id.borrow();
        self.send_once(&toplevel, list, &id, &title, &app_id);
    }

    fn send_once(
        &self,
        toplevel: &Rc<dyn ToplevelNode>,
        list: &ExtForeignToplevelListV1,
        id: &str,
        title: &str,
        app_id: &str,
    ) {
        let handle = match list.publish_toplevel(toplevel) {
            None => return,
            Some(handle) => handle,
        };
        handle.send_identifier(id);
        handle.send_title(title);
        handle.send_app_id(app_id);
        handle.send_done();
        self.handles
            .set((handle.client.id, handle.id), handle.clone());
    }

    pub fn set_title(&self, title: &str) {
        *self.title.borrow_mut() = title.to_string();
        for handle in self.handles.lock().values() {
            handle.send_title(title);
            handle.send_done();
        }
    }

    pub fn set_app_id(&self, app_id: &str) {
        *self.app_id.borrow_mut() = app_id.to_string();
        for handle in self.handles.lock().values() {
            handle.send_app_id(app_id);
            handle.send_done();
        }
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
        if ws.fullscreen.is_some() {
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
        if parent.node_is_workspace() {
            log::warn!("Cannot fullscreen root container in a workspace");
            return;
        }
        let placeholder = Rc::new(PlaceholderNode::new_for(state, node.clone()));
        parent.cnode_replace_child(node.tl_as_node(), placeholder.clone());
        let mut kb_foci = Default::default();
        if ws.visible.get() {
            if let Some(container) = ws.container.get() {
                kb_foci = collect_kb_foci(container);
            }
            for stacked in ws.stacked.iter() {
                collect_kb_foci2(stacked.deref().clone().stacked_into_node(), &mut kb_foci);
            }
        }
        *data = Some(FullscreenedData {
            placeholder,
            workspace: ws.clone(),
        });
        drop(data);
        self.is_fullscreen.set(true);
        node.tl_set_parent(ws.clone());
        ws.set_fullscreen_node(&node);
        node.clone()
            .tl_change_extents(&ws.output.get().global.pos.get());
        for seat in kb_foci {
            node.clone()
                .tl_into_node()
                .node_do_focus(&seat, Direction::Unspecified);
        }
        state.damage();
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
        fd.workspace.remove_fullscreen_node();
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
        state.damage();
    }

    pub fn set_visible(&self, node: &dyn Node, visible: bool) {
        self.visible.set(visible);
        self.seat_state.set_visible(node, visible);
        if !visible {
            return;
        }
        if !self.requested_attention.replace(false) {
            return;
        }
        self.wants_attention.set(false);
        if let Some(parent) = self.parent.get() {
            parent.cnode_child_attention_request_changed(node, false);
        }
        self.state.damage();
    }

    pub fn request_attention(&self, node: &dyn Node) {
        if self.visible.get() {
            return;
        }
        if self.requested_attention.replace(true) {
            return;
        }
        self.wants_attention.set(true);
        if let Some(parent) = self.parent.get() {
            parent.cnode_child_attention_request_changed(node, true);
        }
        self.state.damage();
    }
}
