use {
    crate::{
        client::ClientId,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            jay_workspace::JayWorkspace,
            wl_output::OutputId,
            wl_seat::{tablet::TabletTool, NodeSeatState, WlSeatGlobal},
            wl_surface::{
                x_surface::xwindow::Xwindow, xdg_surface::xdg_toplevel::XdgToplevel, WlSurface,
            },
        },
        rect::Rect,
        renderer::Renderer,
        state::State,
        text::TextTexture,
        tree::{
            container::ContainerNode, walker::NodeVisitor, ContainingNode, Direction,
            FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId, NodeVisitorBase, OutputNode,
            PlaceholderNode, StackedNode, ToplevelNode,
        },
        utils::{
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            linkedlist::{LinkedList, LinkedNode, NodeRef},
            numcell::NumCell,
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
    pub state: Rc<State>,
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
    pub may_capture: Cell<bool>,
    pub has_capture: Cell<bool>,
    pub title_texture: RefCell<Option<TextTexture>>,
    pub attention_requests: ThresholdCounter,
    pub render_highlight: NumCell<u32>,
}

impl WorkspaceNode {
    pub fn clear(&self) {
        self.container.set(None);
        *self.output_link.borrow_mut() = None;
        self.fullscreen.set(None);
        self.jay_workspaces.clear();
    }

    pub fn update_has_captures(&self) {
        let mut has_capture = false;
        let output = self.output.get();
        'update: {
            if !self.may_capture.get() {
                break 'update;
            }
            for sc in output.screencasts.lock().values() {
                if sc.shows_ws(self) {
                    has_capture = true;
                    break 'update;
                }
            }
            if output.screencopies.is_not_empty() {
                has_capture = true;
            }
        }
        if self.has_capture.replace(has_capture) != has_capture {
            output.schedule_update_render_data();
            output.state.damage(output.global.pos.get());
        }
    }

    pub fn set_output(&self, output: &Rc<OutputNode>) {
        self.output.set(output.clone());
        for jw in self.jay_workspaces.lock().values() {
            jw.send_output(output);
        }
        self.update_has_captures();
        struct OutputSetter<'a>(&'a Rc<OutputNode>);
        impl NodeVisitorBase for OutputSetter<'_> {
            fn visit_surface(&mut self, node: &Rc<WlSurface>) {
                node.set_output(self.0);
            }

            fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                node.tl_workspace_output_changed();
                node.node_visit_children(self);
            }

            fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
                node.tl_workspace_output_changed();
                node.node_visit_children(self);
            }

            fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
                node.tl_workspace_output_changed();
                node.node_visit_children(self);
            }

            fn visit_placeholder(&mut self, node: &Rc<PlaceholderNode>) {
                node.tl_workspace_output_changed();
                node.node_visit_children(self);
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
        container.tl_set_visible(self.container_visible());
        self.container.set(Some(container.clone()));
        self.state.damage(self.position.get());
    }

    pub fn is_empty(&self) -> bool {
        self.stacked.is_empty() && self.fullscreen.is_none() && self.container.is_none()
    }

    pub fn container_visible(&self) -> bool {
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

    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        for jw in self.jay_workspaces.lock().values() {
            jw.send_visible(visible);
        }
        for stacked in self.stacked.iter() {
            stacked.stacked_prepare_set_visible();
        }
        if let Some(fs) = self.fullscreen.get() {
            fs.tl_set_visible(visible);
        }
        if let Some(container) = self.container.get() {
            container.tl_set_visible(self.container_visible());
        }
        for stacked in self.stacked.iter() {
            if stacked.stacked_needs_set_visible() {
                stacked.stacked_set_visible(self.container_visible());
            }
        }
        self.seat_state.set_visible(self, visible);
    }

    pub fn set_fullscreen_node(&self, node: &Rc<dyn ToplevelNode>) {
        if let Some(prev) = self.fullscreen.set(Some(node.clone())) {
            self.discard_child_properties(&*prev);
        }
        self.pull_child_properties(&**node);
        if self.visible.get() {
            self.output.get().fullscreen_changed();
        } else {
            node.tl_set_visible(false);
        }
        if let Some(surface) = node.tl_scanout_surface() {
            if let Some(fb) = self.output.get().global.connector.connector.drm_feedback() {
                surface.send_feedback(&fb);
            }
        }
        self.output.get().update_presentation_type();
    }

    pub fn remove_fullscreen_node(&self) {
        if let Some(node) = self.fullscreen.take() {
            self.discard_child_properties(&*node);
            if self.visible.get() {
                self.output.get().fullscreen_changed();
            }
            if let Some(surface) = node.tl_scanout_surface() {
                if let Some(fb) = surface.client.state.drm_feedback.get() {
                    surface.send_feedback(&fb);
                }
            }
            self.output.get().update_presentation_type();
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

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        if let Some(n) = self.container.get() {
            tree.push(FoundNode {
                node: n.clone(),
                x,
                y,
            });
            return n.node_find_tree_at(x, y, tree, usecase);
        }
        FindTreeResult::Other
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, _bounds: Option<&Rect>) {
        renderer.render_workspace(self, x, y);
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("workspace focus");
        seat.pointer_cursor().set_known(KnownCursor::Default);
    }

    fn node_on_tablet_tool_enter(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        _time_usec: u64,
        _x: Fixed,
        _y: Fixed,
    ) {
        tool.cursor().set_known(KnownCursor::Default)
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
                self.state.damage(self.position.get());
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
    if source.node_visible() {
        target.state.damage(source.global.pos.get());
    }
    if target.node_visible() {
        target.state.damage(target.global.pos.get());
    }
}
