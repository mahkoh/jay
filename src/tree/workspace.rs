use {
    crate::{
        client::ClientId,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            jay_workspace::JayWorkspace,
            wl_output::OutputId,
            wl_seat::{NodeSeatState, WlSeatGlobal, tablet::TabletTool},
            wl_surface::{
                WlSurface, x_surface::xwindow::Xwindow, xdg_surface::xdg_toplevel::XdgToplevel,
            },
            workspace_manager::{
                ext_workspace_handle_v1::ExtWorkspaceHandleV1,
                ext_workspace_manager_v1::WorkspaceManagerId,
            },
        },
        rect::Rect,
        renderer::Renderer,
        state::State,
        text::TextTexture,
        tree::{
            ContainingNode, Direction, FindTreeResult, FindTreeUsecase, FloatNode, FoundNode, Node,
            NodeId, NodeLayerLink, NodeLocation, NodeVisitorBase, OutputNode, OutputNodeId,
            PlaceholderNode, StackedNode, ToplevelNode, container::ContainerNode,
            transaction::TreeTransaction, walker::NodeVisitor,
        },
        utils::{
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            linkedlist::{LinkedList, LinkedNode},
            numcell::NumCell,
            opt::Opt,
            threshold_counter::ThresholdCounter,
        },
        wire::JayWorkspaceId,
    },
    jay_config::workspace::WorkspaceDisplayOrder,
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
    pub stacked: LinkedList<Rc<dyn StackedNode>>,
    pub seat_state: NodeSeatState,
    pub name: String,
    pub visible_on_desired_output: Cell<bool>,
    pub desired_output: CloneCell<Rc<OutputId>>,
    pub jay_workspaces: CopyHashMap<(ClientId, JayWorkspaceId), Rc<JayWorkspace>>,
    pub may_capture: Cell<bool>,
    pub has_capture: Cell<bool>,
    pub title_texture: RefCell<Option<TextTexture>>,
    pub attention_requests: ThresholdCounter,
    pub render_highlight: NumCell<u32>,
    pub ext_workspaces: CopyHashMap<WorkspaceManagerId, Rc<ExtWorkspaceHandleV1>>,
    pub opt: Rc<Opt<WorkspaceNode>>,
    pub current: WorkspaceState,
    pub mapped: WorkspaceState,
}

pub struct WorkspaceState {
    pub output: CloneCell<Rc<OutputNode>>,
    pub output_id: Cell<OutputNodeId>,
    pub position: Cell<Rect>,
    pub container: CloneCell<Option<Rc<ContainerNode>>>,
    pub output_link: CloneCell<Option<Rc<LinkedNode<WorkspaceInOutput>>>>,
    pub visible: Cell<bool>,
    pub fullscreen: CloneCell<Option<Rc<dyn ToplevelNode>>>,
}

impl WorkspaceState {
    fn clear(&self) {
        self.container.set(None);
        self.output_link.set(None);
        self.fullscreen.set(None);
    }
}

impl WorkspaceNode {
    pub fn clear(&self) {
        self.current.clear();
        self.mapped.clear();
        self.jay_workspaces.clear();
        self.ext_workspaces.clear();
        self.opt.set(None);
        self.title_texture.take();
    }

    pub fn update_has_captures(&self) {
        let mut has_capture = false;
        let output = self.current.output.get();
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
        self.current.output_id.set(output.id);
        let old = self.current.output.set(output.clone());
        for wh in self.ext_workspaces.lock().values() {
            wh.handle_new_output(output);
        }
        for jw in self.jay_workspaces.lock().values() {
            jw.send_output(output);
        }
        self.update_has_captures();
        struct OutputSetter<'a> {
            ws: &'a WorkspaceNode,
            old: &'a Rc<OutputNode>,
            new: &'a Rc<OutputNode>,
        }
        impl NodeVisitorBase for OutputSetter<'_> {
            fn visit_surface(&mut self, node: &Rc<WlSurface>) {
                node.set_output(self.new, self.ws.location());
            }

            fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                node.tl_workspace_output_changed(self.old, self.new);
                node.node_visit_children(self);
            }

            fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
                node.tl_workspace_output_changed(self.old, self.new);
                node.node_visit_children(self);
            }

            fn visit_float(&mut self, node: &Rc<FloatNode>) {
                node.after_ws_move(self.new);
                node.node_visit_children(self);
            }

            fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
                node.tl_workspace_output_changed(self.old, self.new);
                node.node_visit_children(self);
            }

            fn visit_placeholder(&mut self, node: &Rc<PlaceholderNode>) {
                node.tl_workspace_output_changed(self.old, self.new);
                node.node_visit_children(self);
            }
        }
        let mut visitor = OutputSetter {
            ws: self,
            old: &old,
            new: output,
        };
        self.node_visit_children(&mut visitor);
        for stacked in self.stacked.iter() {
            stacked.deref().clone().node_visit(&mut visitor);
        }
    }

    pub fn set_container(self: &Rc<Self>, tt: &TreeTransaction, container: &Rc<ContainerNode>) {
        if let Some(prev) = self.current.container.get() {
            self.discard_child_properties(&*prev);
        }
        self.pull_child_properties(&**container);
        let pos = self.current.position.get();
        container.clone().tl_change_extents(tt, &pos);
        container.clone().tl_set_parent(tt, self.clone());
        container
            .clone()
            .tl_set_visible(tt, self.container_visible());
        self.current.container.set(Some(container.clone()));
        self.state.damage(self.current.position.get());
    }

    pub fn is_empty(&self) -> bool {
        self.stacked.is_empty()
            && self.current.fullscreen.is_none()
            && self.current.container.is_none()
    }

    pub fn container_visible(&self) -> bool {
        self.current.visible.get() && self.current.fullscreen.is_none()
    }

    pub fn float_visible(&self) -> bool {
        self.current.visible.get()
            && (self.current.fullscreen.is_none() || self.state.float_above_fullscreen.get())
    }

    pub fn change_extents(&self, tt: &TreeTransaction, rect: &Rect) {
        self.current.position.set(*rect);
        if let Some(c) = self.current.container.get() {
            c.tl_change_extents(tt, rect);
        }
    }

    pub fn flush_jay_workspaces(&self) {
        for jw in self.jay_workspaces.lock().values() {
            jw.send_done();
        }
    }

    pub fn set_visible(&self, tt: &TreeTransaction, visible: bool) {
        self.current.visible.set(visible);
        for jw in self.jay_workspaces.lock().values() {
            jw.send_visible(visible);
        }
        for wh in self.ext_workspaces.lock().values() {
            wh.handle_visibility_changed();
        }
        for stacked in self.stacked.iter() {
            stacked.stacked_prepare_set_visible();
        }
        if let Some(fs) = self.current.fullscreen.get() {
            fs.tl_set_visible(tt, visible);
        }
        if let Some(container) = self.current.container.get() {
            container.tl_set_visible(tt, self.container_visible());
        }
        for stacked in self.stacked.iter() {
            if stacked.stacked_needs_set_visible() {
                stacked
                    .deref()
                    .clone()
                    .stacked_set_visible(tt, self.float_visible());
            }
        }
        self.seat_state.set_visible(self, visible);
    }

    pub fn set_fullscreen_node(self: &Rc<Self>, tt: &TreeTransaction, node: &Rc<dyn ToplevelNode>) {
        if let Some(prev) = self.current.fullscreen.set(Some(node.clone())) {
            self.discard_child_properties(&*prev);
        }
        let output = self.current.output.get();
        self.pull_child_properties(&**node);
        if self.current.visible.get() {
            output.fullscreen_changed(tt);
        } else {
            node.clone().tl_set_visible(tt, false);
        }
        if let Some(surface) = node.tl_scanout_surface()
            && let Some(fb) = self
                .current
                .output
                .get()
                .global
                .connector
                .connector
                .drm_feedback()
        {
            surface.send_feedback(&fb);
        }
        self.current.output.get().update_presentation_type(tt);
    }

    pub fn remove_fullscreen_node(self: &Rc<Self>, tt: &TreeTransaction) {
        if let Some(node) = self.current.fullscreen.take() {
            self.discard_child_properties(&*node);
            if self.current.visible.get() {
                self.current.output.get().fullscreen_changed(tt);
            }
            if let Some(surface) = node.tl_scanout_surface()
                && let Some(fb) = surface.client.state.drm_feedback.get()
            {
                surface.send_feedback(&fb);
            }
            self.current.output.get().update_presentation_type(tt);
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
            for wh in self.ext_workspaces.lock().values() {
                wh.handle_urgent_changed();
            }
            self.current.output.get().schedule_update_render_data();
        }
    }

    pub fn location(&self) -> NodeLocation {
        NodeLocation::Workspace(self.current.output_id.get(), self.id)
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
        if let Some(c) = self.current.container.get() {
            visitor.visit_container(&c);
        }
        if let Some(fs) = self.current.fullscreen.get() {
            fs.node_visit(visitor);
        }
    }

    fn node_visible(&self) -> bool {
        self.current.visible.get()
    }

    fn node_mapped_position(&self) -> Rect {
        self.current.position.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        Some(self.current.output.get())
    }

    fn node_location(&self) -> Option<NodeLocation> {
        Some(self.location())
    }

    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Workspace
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        if let Some(fs) = self.current.fullscreen.get() {
            fs.node_do_focus(seat, direction);
        } else if self.stacked.is_not_empty()
            && let Some(last) = seat.get_last_focus_on_workspace(&self)
        {
            seat.focus_node(last);
        } else if let Some(container) = self.current.container.get() {
            container.node_do_focus(seat, direction);
        } else if let Some(float) = self
            .stacked
            .rev_iter()
            .filter_map(|node| (*node).clone().node_into_float())
            .find(|node| node.current.child.is_some())
        {
            if let Some(child) = float.current.child.get() {
                child.node_do_focus(seat, direction);
            }
        }
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        if let Some(n) = self.current.container.get() {
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

    fn node_make_visible(self: Rc<Self>, tt: &TreeTransaction) {
        if self.is_dummy {
            return;
        }
        self.state
            .show_workspace2(tt, None, &self.current.output.get(), &self);
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
    fn cnode_replace_child(
        self: Rc<Self>,
        tt: &TreeTransaction,
        old: &dyn Node,
        new: Rc<dyn ToplevelNode>,
    ) {
        if let Some(container) = self.current.container.get()
            && container.node_id() == old.node_id()
        {
            let new = match new.node_into_container() {
                Some(c) => c,
                _ => {
                    log::error!("cnode_replace_child called with non-container new");
                    return;
                }
            };
            self.set_container(tt, &new);
            return;
        }
        log::error!("Trying to replace child that's not a child");
    }

    fn cnode_remove_child2(
        self: Rc<Self>,
        tt: &TreeTransaction,
        child: &dyn Node,
        _preserve_focus: bool,
    ) {
        if let Some(container) = self.current.container.get()
            && container.node_id() == child.node_id()
        {
            self.discard_child_properties(&*container);
            self.current.container.set(None);
            self.state.damage(self.current.position.get());
            return;
        }
        if let Some(fs) = self.current.fullscreen.get()
            && fs.node_id() == child.node_id()
        {
            self.remove_fullscreen_node(tt);
            return;
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

    fn cnode_make_visible(self: Rc<Self>, tt: &TreeTransaction, _child: &dyn Node) {
        self.node_make_visible(tt);
    }
}

pub struct WsMoveConfig {
    pub make_visible_always: bool,
    pub make_visible_if_empty: bool,
    pub source_is_destroyed: bool,
    pub before: Option<Rc<WorkspaceNode>>,
}

pub fn move_ws_to_output(
    tt: &TreeTransaction,
    ws: &Rc<WorkspaceNode>,
    target: &Rc<OutputNode>,
    config: WsMoveConfig,
) {
    let source = ws.current.output.get();
    if let Some(visible) = source.workspace.get()
        && visible.id == ws.id
    {
        source.workspace.take();
    }
    let mut new_source_ws = None;
    if !config.source_is_destroyed && !source.is_dummy && source.workspace.is_none() {
        new_source_ws = source
            .workspaces
            .iter()
            .find(|c| c.id != ws.id)
            .map(|c| (*c).clone());
        if new_source_ws.is_none() && source.pinned.is_not_empty() {
            new_source_ws = Some(source.generate_workspace(tt));
        }
    }
    if let Some(new_source_ws) = &new_source_ws {
        for pinned in source.pinned.iter() {
            pinned
                .deref()
                .clone()
                .set_workspace(tt, new_source_ws, false);
        }
    }
    ws.set_output(&target);
    let before = if target.state.workspace_display_order.get() == WorkspaceDisplayOrder::Sorted {
        target
            .find_workspace_insertion_point(&ws.name)
            .map(|nr| nr.ws.clone())
    } else {
        config.before
    };
    let wio = WorkspaceInOutput::new(&ws);
    let link = if let Some(before) = before
        && before.current.output_id.get() == target.id
        && let Some(link) = before.current.output_link.get()
    {
        link.prepend(wio)
    } else {
        target.workspaces.add_last(wio)
    };
    ws.current.output_link.set(Some(Rc::new(link)));
    let make_visible = !target.is_dummy
        && (config.make_visible_always
            || (config.make_visible_if_empty && target.workspace.is_none()));
    if make_visible {
        ws.state.show_workspace2(tt, None, target, &ws);
    } else {
        ws.set_visible(tt, false);
    }
    ws.flush_jay_workspaces();
    if let Some(ws) = new_source_ws {
        ws.state.show_workspace2(tt, None, &source, &ws);
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

pub struct WorkspaceInOutput {
    pub ws: Rc<WorkspaceNode>,
    pub is_current_link: Cell<bool>,
    pub is_mapped_link: Cell<bool>,
}

impl WorkspaceInOutput {
    pub fn new(ws: &Rc<WorkspaceNode>) -> Self {
        Self {
            ws: ws.clone(),
            is_current_link: Cell::new(true),
            is_mapped_link: Cell::new(true),
        }
    }
}

impl Deref for WorkspaceInOutput {
    type Target = Rc<WorkspaceNode>;

    fn deref(&self) -> &Self::Target {
        &self.ws
    }
}

pub struct WorkspaceDragDestination {
    pub highlight: Rect,
    pub output: Rc<OutputNode>,
    pub before: Option<Rc<WorkspaceNode>>,
}
