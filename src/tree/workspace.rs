use {
    crate::{
        client::ClientId,
        control_center::CCI_WORKSPACES,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            jay_workspace::JayWorkspace,
            wl_output::OutputId,
            wl_seat::{NodeSeatState, WlSeatGlobal, collect_kb_foci2, tablet::TabletTool},
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
        transactions::{TransactionData, Transactionable, TransactionableExt},
        tree::{
            ContainingNode, Direction, FindTreeResult, FindTreeUsecase, FloatNode, FoundNode, Node,
            NodeBase, NodeId, NodeLayerLink, NodeLocation, NodeVisitorBase, OutputNode,
            PlaceholderNode, SplitView, StackedNode, ToplevelNode,
            TreeTimeline::{self, LiveTL, RenderTL},
            WorkspaceDisplayOrder,
            container::ContainerNode,
            walker::NodeVisitor,
        },
        utils::{
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            linkedlist::{LinkedList, LinkedNode, NodeRef},
            numcell::NumCell,
            obj_and_id::{ObjAndId, ObjWithId},
            opt::Opt,
            threshold_counter::ThresholdCounter,
        },
        wire::JayWorkspaceId,
    },
    linearize::Linearize,
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        fmt::Debug,
        ops::Deref,
        rc::Rc,
    },
};

tree_id!(WorkspaceNodeId);

hash_type!(WorkspaceNameHash);

#[derive(Copy, Clone, Linearize, Eq, PartialEq, Debug)]
pub enum WorkspaceType {
    Normal,
    Overlay,
}

pub struct WorkspaceNode {
    pub id: WorkspaceNodeId,
    pub state: Rc<State>,
    pub ty: WorkspaceType,
    pub stacked: LinkedList<Rc<dyn StackedNode>>,
    pub seat_state: NodeSeatState,
    pub name: Rc<String>,
    pub name_hash: WorkspaceNameHash,
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
    pub node_state: SplitView<WorkspaceNodeState>,
    pub output_link: Cell<Option<LinkedNode<WorkspaceOutputLink>>>,
    pub transaction_data: TransactionData<WorkspaceTransactionOp>,
}

pub struct WorkspaceNodeState {
    pub output: ObjAndId<Rc<OutputNode>>,
    pub position: Cell<Rect>,
    pub container: CloneCell<Option<Rc<ContainerNode>>>,
    pub output_link: CloneCell<Option<NodeRef<WorkspaceOutputLink>>>,
    pub visible: Cell<bool>,
    pub fullscreen: CloneCell<Option<Rc<dyn ToplevelNode>>>,
}

pub struct WorkspaceOutputLink {
    pub ws: Rc<WorkspaceNode>,
}

impl Deref for WorkspaceOutputLink {
    type Target = Rc<WorkspaceNode>;

    fn deref(&self) -> &Self::Target {
        &self.ws
    }
}

impl ObjWithId for Rc<WorkspaceNode> {
    type Id = WorkspaceNodeId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl WorkspaceNode {
    pub fn new(output: &Rc<OutputNode>, name: &str, ty: WorkspaceType) -> Rc<Self> {
        let slf = Rc::new(Self {
            id: output.state.node_ids.next(),
            state: output.state.clone(),
            ty,
            stacked: Default::default(),
            seat_state: Default::default(),
            name: Rc::new(name.to_string()),
            name_hash: WorkspaceNameHash::hash(name),
            visible_on_desired_output: Default::default(),
            desired_output: CloneCell::new(output.global.output_id.clone()),
            jay_workspaces: Default::default(),
            may_capture: output.state.default_workspace_capture.clone(),
            has_capture: Default::default(),
            title_texture: Default::default(),
            attention_requests: Default::default(),
            render_highlight: Default::default(),
            ext_workspaces: Default::default(),
            opt: Default::default(),
            node_state: SplitView::from_fn(|_| WorkspaceNodeState::new(output)),
            output_link: Default::default(),
            transaction_data: TransactionData::new(&output.state.tree),
        });
        slf.seat_state.disable_focus_history();
        slf
    }

    pub fn clear(self: &Rc<Self>) {
        self.seat_state.destroy_node(&**self);
        self.set_ns_container(None);
        self.set_ns_output_link(None);
        self.set_ns_fullscreen(None);
        self.jay_workspaces.clear();
        self.ext_workspaces.clear();
        self.opt.set(None);
        self.add_transaction_op(WorkspaceTransactionOp::ClearTitleTexture);
    }

    pub fn update_has_captures(&self) {
        if self.ty != WorkspaceType::Normal {
            return;
        }
        let mut has_capture = false;
        let output = self.node_state[LiveTL].output.get();
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
            output.state.damage(output.node_state[LiveTL].pos.get());
        }
    }

    pub fn set_output(self: &Rc<Self>, output: &Rc<OutputNode>) {
        let old = self.set_ns_output(output);
        if let Some(tl) = self.node_state[LiveTL].fullscreen.get() {
            tl.tl_mark_fullscreen(Some(&output.global.connector));
        }
        for wh in self.ext_workspaces.lock().values() {
            wh.handle_new_output(output);
        }
        for jw in self.jay_workspaces.lock().values() {
            jw.send_output(output);
        }
        self.update_has_captures();
        self.change_extents(&output.node_state[LiveTL].rects.workspace.get(), output);
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
                if self.ws.ty == WorkspaceType::Normal {
                    node.after_ws_move(self.new);
                }
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
            stacked.deref().clone().node_visit_dyn(&mut visitor);
        }
        self.state.trigger_cci(CCI_WORKSPACES);
    }

    pub fn set_container(self: &Rc<Self>, container: &Rc<ContainerNode>) {
        let ns = &self.node_state[LiveTL];
        if let Some(prev) = ns.container.get() {
            self.discard_child_properties(&*prev);
        }
        self.pull_child_properties(&**container);
        let pos = ns.position.get();
        container.clone().tl_change_extents(&pos);
        container.tl_set_parent(self.clone());
        container.tl_set_visible(self.container_visible());
        self.set_ns_container(Some(container));
        self.state.damage(ns.position.get());
    }

    pub fn is_empty(&self) -> bool {
        let ns = &self.node_state[LiveTL];
        self.stacked.is_empty() && ns.fullscreen.is_none() && ns.container.is_none()
    }

    pub fn container_visible(&self) -> bool {
        let ns = &self.node_state[LiveTL];
        ns.visible.get() && ns.fullscreen.is_none()
    }

    pub fn float_visible(&self) -> bool {
        let ns = &self.node_state[LiveTL];
        ns.visible.get() && (ns.fullscreen.is_none() || self.state.float_above_fullscreen.get())
    }

    pub fn change_extents(self: &Rc<Self>, rect: &Rect, output: &Rc<OutputNode>) {
        if output.is_dummy {
            return;
        }
        let ns = &self.node_state[LiveTL];
        let old = self.set_ns_position(*rect);
        if let Some(c) = ns.container.get() {
            c.tl_change_extents(rect);
        }
        if old != *rect {
            let mut dx = 0;
            let mut dy = 0;
            if old.is_not_empty() {
                dx = rect.x1() - old.x1();
                dy = rect.y1() - old.y1();
            }
            for stacked in self.stacked.iter() {
                if let Some(float) = stacked.deref().clone().node_into_float() {
                    if (dx, dy) != (0, 0) {
                        float.move_(dx, dy);
                    }
                    float.ensure_on_output(&output);
                }
            }
        }
        self.state.trigger_cci(CCI_WORKSPACES);
    }

    pub fn flush_jay_workspaces(&self) {
        for jw in self.jay_workspaces.lock().values() {
            jw.send_done();
        }
    }

    pub fn set_visible(self: &Rc<Self>, visible: bool) {
        let ns = &self.node_state[LiveTL];
        self.set_ns_visible(visible);
        for jw in self.jay_workspaces.lock().values() {
            jw.send_visible(visible);
        }
        for wh in self.ext_workspaces.lock().values() {
            wh.handle_visibility_changed();
        }
        for stacked in self.stacked.iter() {
            stacked.stacked_prepare_set_visible();
        }
        if let Some(fs) = ns.fullscreen.get() {
            fs.tl_set_visible(visible);
        }
        if let Some(container) = ns.container.get() {
            container.tl_set_visible(self.container_visible());
        }
        for stacked in self.stacked.iter() {
            if stacked.stacked_needs_set_visible() {
                stacked
                    .deref()
                    .clone()
                    .stacked_set_visible(self.float_visible());
            }
        }
        self.seat_state.set_visible(&**self, visible);
        self.state.trigger_cci(CCI_WORKSPACES);
    }

    pub fn set_fullscreen_node(self: &Rc<Self>, node: &Rc<dyn ToplevelNode>) {
        let ns = &self.node_state[LiveTL];
        if let Some(prev) = self.set_ns_fullscreen(Some(node)) {
            self.discard_child_properties(&*prev);
        }
        self.pull_child_properties(&**node);
        if ns.visible.get() {
            ns.output.get().fullscreen_changed();
        } else {
            node.tl_set_visible(false);
        }
        ns.output.get().update_presentation_type();
    }

    pub fn remove_fullscreen_node(self: &Rc<Self>) {
        let ns = &self.node_state[LiveTL];
        if let Some(node) = self.set_ns_fullscreen(None) {
            self.discard_child_properties(&*node);
            if ns.visible.get() {
                ns.output.get().fullscreen_changed();
            }
            ns.output.get().update_presentation_type();
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
            self.node_state[LiveTL]
                .output
                .get()
                .schedule_update_render_data();
        }
    }

    pub fn location(&self) -> NodeLocation {
        NodeLocation::Workspace(self.node_state[LiveTL].output.id(), self.id)
    }

    pub fn collect_kb_foci(self: &Rc<Self>) -> SmallVec<[Rc<WlSeatGlobal>; 3]> {
        let mut seats = SmallVec::new();
        self.collect_kb_foci2(&mut seats);
        seats
    }

    pub fn collect_kb_foci2(self: &Rc<Self>, seats: &mut SmallVec<[Rc<WlSeatGlobal>; 3]>) {
        collect_kb_foci2(self.clone(), seats);
        for node in self.stacked.iter() {
            collect_kb_foci2(node.deref().clone(), seats);
        }
    }

    pub fn do_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) -> bool {
        let ns = &self.node_state[LiveTL];
        if let Some(fs) = ns.fullscreen.get() {
            fs.node_do_focus_dyn(seat, direction);
        } else if self.stacked.is_not_empty()
            && let Some(last) = seat.get_last_focus_on_workspace(&self)
        {
            seat.focus_node(last);
        } else if let Some(container) = ns.container.get() {
            container.node_do_focus(seat, direction);
        } else if let Some(child) = self
            .stacked
            .rev_iter()
            .filter_map(|node| (*node).clone().node_into_float())
            .find_map(|float| float.node_state[LiveTL].child.get())
        {
            child.node_do_focus_dyn(seat, direction);
        } else if self.ty == WorkspaceType::Normal {
            seat.focus_node(self.clone());
        } else {
            return false;
        }
        true
    }

    fn set_ns_output(self: &Rc<Self>, v: &Rc<OutputNode>) -> Rc<OutputNode> {
        self.add_transaction_op(WorkspaceTransactionOp::SetOutput(v.clone()));
        self.node_state[LiveTL].output.set(v.clone())
    }

    fn set_ns_position(self: &Rc<Self>, v: Rect) -> Rect {
        self.add_transaction_op(WorkspaceTransactionOp::SetPosition(v));
        self.node_state[LiveTL].position.replace(v)
    }

    fn set_ns_container(self: &Rc<Self>, v: Option<&Rc<ContainerNode>>) {
        self.add_transaction_op(WorkspaceTransactionOp::SetContainer(v.cloned()));
        self.node_state[LiveTL].container.set(v.cloned());
    }

    pub fn set_ns_output_link(self: &Rc<Self>, v: Option<LinkedNode<WorkspaceOutputLink>>) {
        let ref_ = v.as_ref().map(|v| v.to_ref());
        self.add_transaction_op(WorkspaceTransactionOp::SetOutputLink(ref_.clone()));
        self.node_state[LiveTL].output_link.set(ref_);
        self.output_link.set(v);
    }

    fn set_ns_visible(self: &Rc<Self>, v: bool) {
        self.add_transaction_op(WorkspaceTransactionOp::SetVisible(v));
        self.node_state[LiveTL].visible.set(v);
    }

    fn set_ns_fullscreen(
        self: &Rc<Self>,
        v: Option<&Rc<dyn ToplevelNode>>,
    ) -> Option<Rc<dyn ToplevelNode>> {
        self.add_transaction_op(WorkspaceTransactionOp::SetFullscreen(v.cloned()));
        self.node_state[LiveTL].fullscreen.set(v.cloned())
    }
}

impl NodeBase for WorkspaceNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_workspace(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let ns = &self.node_state[LiveTL];
        if let Some(c) = ns.container.get() {
            visitor.visit_container(&c);
        }
        if let Some(fs) = ns.fullscreen.get() {
            fs.node_visit_dyn(visitor);
        }
    }

    fn node_visible(&self, tl: TreeTimeline) -> bool {
        self.node_state[tl].visible.get()
    }

    fn node_absolute_position(&self, tl: TreeTimeline) -> Rect {
        self.node_state[tl].position.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        Some(self.node_state[LiveTL].output.get())
    }

    fn node_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        self.opt.get()
    }

    fn node_location(&self) -> Option<NodeLocation> {
        Some(self.location())
    }

    fn node_layer(&self) -> NodeLayerLink {
        if self.ty == WorkspaceType::Overlay {
            return NodeLayerLink::Overlay;
        }
        NodeLayerLink::Workspace
    }

    fn node_do_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        self.do_focus(seat, direction);
    }

    fn node_active_changed(&self, _active: bool) {
        let output = self.node_state[LiveTL].output.get();
        self.state
            .damage(output.node_state[LiveTL].rects.bar_separator.get());
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        if let Some(n) = self.node_state[LiveTL].container.get() {
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

    fn node_make_visible(self: &Rc<Self>) {
        if self.ty != WorkspaceType::Normal {
            return;
        }
        self.state
            .show_workspace2(None, &self.node_state[LiveTL].output.get(), self);
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
        if let Some(container) = self.node_state[LiveTL].container.get()
            && container.node_id() == old.node_id()
        {
            let new = match new.node_into_container() {
                Some(c) => c,
                _ => {
                    log::error!("cnode_replace_child called with non-container new");
                    return;
                }
            };
            self.set_container(&new);
            return;
        }
        log::error!("Trying to replace child that's not a child");
    }

    fn cnode_remove_child2(self: Rc<Self>, child: &dyn Node, _preserve_focus: bool) {
        let ns = &self.node_state[LiveTL];
        if let Some(container) = ns.container.get()
            && container.node_id() == child.node_id()
        {
            self.discard_child_properties(&*container);
            self.set_ns_container(None);
            self.state.damage(ns.position.get());
            return;
        }
        if let Some(fs) = ns.fullscreen.get()
            && fs.node_id() == child.node_id()
        {
            self.remove_fullscreen_node();
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

    fn cnode_make_visible(self: Rc<Self>, _child: &dyn Node) {
        self.node_make_visible();
    }
}

pub struct WsMoveConfig {
    pub make_visible_always: bool,
    pub make_visible_if_empty: bool,
    pub source_is_destroyed: bool,
    pub before: Option<Rc<WorkspaceNode>>,
}

pub fn move_ws_to_output(ws: &Rc<WorkspaceNode>, target: &Rc<OutputNode>, config: WsMoveConfig) {
    if ws.ty == WorkspaceType::Overlay {
        target.show_workspace(&ws);
        return;
    }
    let ns = &ws.node_state[LiveTL];
    if ns.output_link.is_none() {
        return;
    }
    let source = ns.output.get();
    let sns = &source.node_state[LiveTL];
    if let Some(visible) = sns.workspace.id()
        && visible == ws.id
    {
        source.set_ns_workspace(None);
    }
    let mut new_source_ws = None;
    if !config.source_is_destroyed && !source.is_dummy && sns.workspace.is_none() {
        new_source_ws = source
            .workspaces
            .iter()
            .find(|c| c.id != ws.id)
            .map(|c| c.ws.clone());
        if new_source_ws.is_none() && source.pinned.is_not_empty() {
            new_source_ws = Some(source.generate_normal_workspace());
        }
    }
    if sns.overlay.is_none() {
        for user in source.cursor_users.lock().values() {
            user.workspace_changed(&source, new_source_ws.as_ref());
            if new_source_ws.is_none() {
                new_source_ws = sns.workspace.get();
            }
        }
    }
    if let Some(new_source_ws) = &new_source_ws {
        for pinned in source.pinned.iter() {
            pinned.deref().clone().set_workspace(new_source_ws, false);
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
    let link = {
        let data = WorkspaceOutputLink { ws: ws.clone() };
        if let Some(before) = before
            && let Some(link) = before.node_state[LiveTL].output_link.get()
        {
            link.prepend(data)
        } else {
            target.workspaces.add_last(data)
        }
    };
    ws.set_ns_output_link(Some(link));
    let tns = &target.node_state[LiveTL];
    let make_visible = !target.is_dummy
        && (config.make_visible_always
            || (config.make_visible_if_empty && tns.workspace.is_none()));
    if make_visible {
        ws.state.show_workspace2(None, target, &ws);
    } else {
        ws.set_visible(false);
    }
    ws.flush_jay_workspaces();
    if let Some(ws) = new_source_ws {
        ws.state.show_workspace2(None, &source, &ws);
    }
    if !target.is_dummy {
        target.schedule_update_render_data();
    }
    if !source.is_dummy {
        source.schedule_update_render_data();
    }
    if source.node_visible(LiveTL) {
        target.state.damage(sns.pos.get());
    }
    if target.node_visible(LiveTL) {
        target.state.damage(tns.pos.get());
    }
}

pub struct WorkspaceDragDestination {
    pub highlight: Rect,
    pub output: Rc<OutputNode>,
    pub before: Option<Rc<WorkspaceNode>>,
}

impl WorkspaceNodeState {
    fn new(output: &Rc<OutputNode>) -> Self {
        Self {
            output: ObjAndId::new(output.clone()),
            position: Default::default(),
            container: Default::default(),
            output_link: Default::default(),
            visible: Default::default(),
            fullscreen: Default::default(),
        }
    }
}

pub enum WorkspaceTransactionOp {
    SetOutput(Rc<OutputNode>),
    SetPosition(Rect),
    SetContainer(Option<Rc<ContainerNode>>),
    SetOutputLink(Option<NodeRef<WorkspaceOutputLink>>),
    SetVisible(bool),
    SetFullscreen(Option<Rc<dyn ToplevelNode>>),
    ClearTitleTexture,
}

impl Transactionable for WorkspaceNode {
    type T = WorkspaceTransactionOp;

    fn data(&self) -> &TransactionData<Self::T> {
        &self.transaction_data
    }

    fn apply(self: &Rc<Self>, op: Self::T) {
        let s = &self.node_state[RenderTL];
        match op {
            WorkspaceTransactionOp::SetOutput(v) => {
                s.output.set(v);
            }
            WorkspaceTransactionOp::SetPosition(v) => {
                s.position.set(v);
            }
            WorkspaceTransactionOp::SetContainer(v) => {
                s.container.set(v);
            }
            WorkspaceTransactionOp::SetOutputLink(v) => {
                s.output_link.set(v);
            }
            WorkspaceTransactionOp::SetVisible(v) => {
                s.visible.set(v);
            }
            WorkspaceTransactionOp::SetFullscreen(v) => {
                s.fullscreen.set(v);
            }
            WorkspaceTransactionOp::ClearTitleTexture => {
                self.title_texture.take();
            }
        }
    }
}
