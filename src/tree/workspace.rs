use {
    crate::{
        client::ClientId,
        cursor::KnownCursor,
        ifs::{
            jay_workspace::JayWorkspace,
            wl_output::OutputId,
            wl_seat::{NodeSeatState, WlSeatGlobal},
            wl_surface::WlSurface,
        },
        rect::Rect,
        renderer::Renderer,
        text::TextTexture,
        tree::{
            container::ContainerNode, walker::NodeVisitor, ContainingNode, Direction,
            FindTreeResult, FoundNode, Node, NodeId, NodeVisitorBase, OutputNode, StackedNode,
            ToplevelNode,
        },
        utils::{
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            linkedlist::{LinkedList, LinkedNode, NodeRef},
            threshold_counter::ThresholdCounter,
        },
        wire::JayWorkspaceId,
    },
    std::{
        cell::{Cell, RefCell},
        fmt::Debug,
        ops::Deref,
        rc::Rc,
    },
};

tree_id!(WorkspaceNodeId);

pub struct WorkspaceNode {
    pub id: WorkspaceNodeId,
    pub is_dummy: bool,
    pub output: CloneCell<Rc<OutputNode>>,
    pub position: Cell<Rect>,
    pub container: CloneCell<Option<Rc<ContainerNode>>>,
    pub stacked: LinkedList<Rc<dyn StackedNode>>,
    pub seat_state: NodeSeatState,
    pub name: String,
    pub output_link: RefCell<Option<LinkedNode<Rc<WorkspaceNode>>>>,
    pub visible: Cell<bool>,
    pub fullscreen: CloneCell<Option<Rc<dyn ToplevelNode>>>,
    pub visible_on_desired_output: Cell<bool>,
    pub desired_output: CloneCell<Rc<OutputId>>,
    pub jay_workspaces: CopyHashMap<(ClientId, JayWorkspaceId), Rc<JayWorkspace>>,
    pub capture: Cell<bool>,
    pub title_texture: Cell<Option<TextTexture>>,
    pub attention_requests: ThresholdCounter,
}

impl WorkspaceNode {
    pub fn clear(&self) {
        self.container.set(None);
        *self.output_link.borrow_mut() = None;
        self.fullscreen.set(None);
        self.jay_workspaces.clear();
    }

    pub fn set_output(&self, output: &Rc<OutputNode>) {
        self.output.set(output.clone());
        for jw in self.jay_workspaces.lock().values() {
            jw.send_output(output);
        }
        struct OutputSetter<'a>(&'a Rc<OutputNode>);
        impl NodeVisitorBase for OutputSetter<'_> {
            fn visit_surface(&mut self, node: &Rc<WlSurface>) {
                node.set_output(self.0);
            }
        }
        let mut visitor = OutputSetter(output);
        self.node_visit_children(&mut visitor);
        for stacked in self.stacked.iter() {
            stacked.deref().clone().node_visit(&mut visitor);
        }
    }

    pub fn set_container(self: &Rc<Self>, container: &Rc<ContainerNode>) {
        if let Some(prev) = self.container.get() {
            self.discard_child_properties(&*prev);
        }
        self.pull_child_properties(&**container);
        let pos = self.position.get();
        container.clone().tl_change_extents(&pos);
        container.tl_set_parent(self.clone());
        container.tl_set_visible(self.stacked_visible());
        self.container.set(Some(container.clone()));
    }

    pub fn is_empty(&self) -> bool {
        self.stacked.is_empty() && self.fullscreen.is_none() && self.container.is_none()
    }

    pub fn stacked_visible(&self) -> bool {
        self.visible.get() && self.fullscreen.is_none()
    }

    pub fn change_extents(&self, rect: &Rect) {
        self.position.set(*rect);
        if let Some(c) = self.container.get() {
            c.tl_change_extents(rect);
        }
    }

    pub fn flush_jay_workspaces(&self) {
        for jw in self.jay_workspaces.lock().values() {
            jw.send_done();
        }
    }

    fn plane_set_visible(&self, visible: bool) {
        if let Some(container) = self.container.get() {
            container.tl_set_visible(visible);
        }
        for stacked in self.stacked.iter() {
            stacked.stacked_set_visible(visible);
        }
    }

    pub fn set_visible(&self, visible: bool) {
        for jw in self.jay_workspaces.lock().values() {
            jw.send_visible(visible);
        }
        self.visible.set(visible);
        if let Some(fs) = self.fullscreen.get() {
            fs.tl_set_visible(visible);
        } else {
            self.plane_set_visible(visible);
        }
        self.seat_state.set_visible(self, visible);
    }

    pub fn set_fullscreen_node(&self, node: &Rc<dyn ToplevelNode>) {
        let visible = self.visible.get();
        let mut plane_was_visible = visible;
        if let Some(prev) = self.fullscreen.set(Some(node.clone())) {
            plane_was_visible = false;
            self.discard_child_properties(&*prev);
        }
        self.pull_child_properties(&**node);
        node.tl_set_visible(visible);
        if plane_was_visible {
            self.plane_set_visible(false);
        }
        if let Some(surface) = node.tl_scanout_surface() {
            if let Some(fb) = self.output.get().global.connector.connector.drm_feedback() {
                surface.send_feedback(&fb);
            }
        }
    }

    pub fn remove_fullscreen_node(&self) {
        if let Some(node) = self.fullscreen.take() {
            self.discard_child_properties(&*node);
            if self.visible.get() {
                self.plane_set_visible(true);
            }
            if let Some(surface) = node.tl_scanout_surface() {
                if let Some(fb) = surface.client.state.drm_feedback.get() {
                    surface.send_feedback(&fb);
                }
            }
        }
    }

    fn pull_child_properties(&self, child: &dyn ToplevelNode) {
        if child.tl_data().wants_attention.get() {
            self.mod_attention_requested(true);
        }
    }

    fn discard_child_properties(&self, child: &dyn ToplevelNode) {
        if child.tl_data().wants_attention.get() {
            self.mod_attention_requested(false);
        }
    }

    fn mod_attention_requested(&self, set: bool) {
        let crossed_threshold = self.attention_requests.adj(set);
        if crossed_threshold {
            self.output.get().schedule_update_render_data();
        }
    }
}

impl Node for WorkspaceNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_workspace(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.container.get() {
            visitor.visit_container(&c);
        }
        if let Some(fs) = self.fullscreen.get() {
            fs.tl_into_node().node_visit(visitor);
        }
    }

    fn node_visible(&self) -> bool {
        self.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.position.get()
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        if let Some(fs) = self.fullscreen.get() {
            fs.tl_into_node().node_do_focus(seat, direction);
        } else if let Some(container) = self.container.get() {
            container.node_do_focus(seat, direction);
        }
    }

    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        if let Some(n) = self.container.get() {
            tree.push(FoundNode {
                node: n.clone(),
                x,
                y,
            });
            n.node_find_tree_at(x, y, tree);
        }
        FindTreeResult::AcceptsInput
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, _bounds: Option<&Rect>) {
        renderer.render_workspace(self, x, y);
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("workspace focus");
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn node_into_workspace(self: Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        Some(self.clone())
    }

    fn node_into_containing_node(self: Rc<Self>) -> Option<Rc<dyn ContainingNode>> {
        Some(self)
    }

    fn node_is_workspace(&self) -> bool {
        true
    }
}

impl ContainingNode for WorkspaceNode {
    containing_node_impl!();

    fn cnode_replace_child(self: Rc<Self>, old: &dyn Node, new: Rc<dyn ToplevelNode>) {
        if let Some(container) = self.container.get() {
            if container.node_id() == old.node_id() {
                let new = match new.tl_into_node().node_into_container() {
                    Some(c) => c,
                    _ => {
                        log::error!("cnode_replace_child called with non-container new");
                        return;
                    }
                };
                self.set_container(&new);
                return;
            }
        }
        log::error!("Trying to replace child that's not a child");
    }

    fn cnode_remove_child2(self: Rc<Self>, child: &dyn Node, _preserve_focus: bool) {
        if let Some(container) = self.container.get() {
            if container.node_id() == child.node_id() {
                self.discard_child_properties(&*container);
                self.container.set(None);
                return;
            }
        }
        if let Some(fs) = self.fullscreen.get() {
            if fs.tl_as_node().node_id() == child.node_id() {
                self.remove_fullscreen_node();
                return;
            }
        }
        log::error!("Trying to remove child that's not a child");
    }

    fn cnode_accepts_child(&self, node: &dyn Node) -> bool {
        node.node_is_container()
    }

    fn cnode_child_attention_request_changed(self: Rc<Self>, _node: &dyn Node, set: bool) {
        self.mod_attention_requested(set);
    }

    fn cnode_workspace(self: Rc<Self>) -> Rc<WorkspaceNode> {
        self
    }
}

pub struct WsMoveConfig {
    pub make_visible_if_empty: bool,
    pub source_is_destroyed: bool,
}

pub fn move_ws_to_output(
    ws: &NodeRef<Rc<WorkspaceNode>>,
    target: &Rc<OutputNode>,
    config: WsMoveConfig,
) {
    let source = ws.output.get();
    ws.set_output(&target);
    target.workspaces.add_last_existing(&ws);
    if config.make_visible_if_empty && target.workspace.is_none() && !target.is_dummy {
        target.show_workspace(&ws);
    } else {
        ws.set_visible(false);
    }
    ws.flush_jay_workspaces();
    if let Some(visible) = source.workspace.get() {
        if visible.id == ws.id {
            source.workspace.take();
        }
    }
    if !config.source_is_destroyed && !source.is_dummy {
        if source.workspace.is_none() {
            if let Some(ws) = source.workspaces.first() {
                source.show_workspace(&ws);
                ws.flush_jay_workspaces();
            }
        }
    }
    if !target.is_dummy {
        target.schedule_update_render_data();
    }
    if !source.is_dummy {
        source.schedule_update_render_data();
    }
}
