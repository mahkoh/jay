use {
    crate::{
        backend::ConnectorId,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::wl_seat::{NodeSeatState, WlSeatGlobal, tablet::TabletTool},
        rect::Rect,
        renderer::Renderer,
        state::{State, TreeState},
        transactions::{TransactionData, Transactionable, TransactionableExt},
        tree::{
            FindTreeResult, FindTreeUsecase, FoundNode, NodeBase, NodeId, NodeLayerLink,
            NodeLocation, OutputNode, SplitView, StackedNode, TileDragDestination,
            TreeTimeline::{self, LiveTL, RenderTL},
            WorkspaceDragDestination, WorkspaceNode,
            walker::NodeVisitor,
        },
        utils::{
            copyhashmap::CopyHashMap,
            linkedlist::{LinkedList, LinkedListIter, LinkedNode, NodeRef, RevLinkedListIter},
        },
    },
    linearize::LinearizeExt,
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
};

pub struct DisplayNode {
    pub id: NodeId,
    pub node_state: SplitView<DisplayNodeState>,
    pub outputs: CopyHashMap<ConnectorId, Rc<OutputNode>>,
    pub stacked: Rc<NodesStack>,
    pub stacked_above_layers: Rc<NodesStack>,
    pub stacked_in_overlay: Rc<NodesStack>,
    pub seat_state: NodeSeatState,
    pub transaction_data: TransactionData<DisplayTransactionOp>,
}

#[derive(Default)]
pub struct DisplayNodeState {
    pub extents: Cell<Rect>,
}

#[derive(Default)]
pub struct NodesStack {
    pub stacked: LinkedList<Rc<dyn StackedNode>>,
    visible: SplitView<LinkedList<Rc<dyn StackedNode>>>,
    visible_valid: SplitView<Cell<bool>>,
}

pub struct NodesStackElement {
    pub stack: Rc<NodesStack>,
    pub link: Option<LinkedNode<Rc<dyn StackedNode>>>,
    visible: SplitView<Option<LinkedNode<Rc<dyn StackedNode>>>>,
}

impl DisplayNode {
    pub fn new(tree: &Rc<TreeState>, id: NodeId) -> Self {
        let slf = Self {
            id,
            node_state: Default::default(),
            outputs: Default::default(),
            stacked: Default::default(),
            stacked_above_layers: Default::default(),
            stacked_in_overlay: Default::default(),
            seat_state: Default::default(),
            transaction_data: TransactionData::new(tree),
        };
        slf.seat_state.disable_focus_history();
        slf
    }

    pub fn clear(&self) {
        self.outputs.clear();
        self.seat_state.clear();
    }

    pub fn update_extents(self: &Rc<Self>) {
        let outputs = self.outputs.lock();
        let mut x1 = i32::MAX;
        let mut y1 = i32::MAX;
        let mut x2 = i32::MIN;
        let mut y2 = i32::MIN;
        for output in outputs.values() {
            let pos = output.node_state[LiveTL].pos.get();
            x1 = x1.min(pos.x1());
            y1 = y1.min(pos.y1());
            x2 = x2.max(pos.x2());
            y2 = y2.max(pos.y2());
        }
        if x1 >= x2 {
            x1 = 0;
            x2 = 0;
        }
        if y1 >= y2 {
            y1 = 0;
            y2 = 0;
        }
        let v = Rect::new_saturating(x1, y1, x2, y2);
        self.node_state[LiveTL].extents.set(v);
        self.add_transaction_op(DisplayTransactionOp::SetExtents(v));
    }

    pub fn update_visible(&self, state: &Rc<State>) {
        let visible = state.root_visible();
        for output in self.outputs.lock().values() {
            output.update_visible();
        }
        for layer in [
            &self.stacked,
            &self.stacked_above_layers,
            &self.stacked_in_overlay,
        ] {
            for stacked in layer.stacked.iter() {
                if !stacked.stacked_has_workspace_link() {
                    stacked.deref().clone().stacked_set_visible(visible);
                }
            }
        }
        for seat in state.globals.seats.lock().values() {
            seat.set_visible(visible);
        }
        if visible {
            state.damage_full(LiveTL);
        }
    }

    pub fn tile_drag_destination(
        &self,
        source: NodeId,
        x: i32,
        y: i32,
    ) -> Option<TileDragDestination> {
        for output in self.outputs.lock().values() {
            let pos = output.node_absolute_position(LiveTL);
            if pos.contains(x, y) {
                return output.tile_drag_destination(source, x, y);
            }
        }
        None
    }

    pub fn workspace_drag_destination(
        &self,
        source: &WorkspaceNode,
        x: i32,
        y: i32,
    ) -> Option<WorkspaceDragDestination> {
        for output in self.outputs.lock().values() {
            let pos = output.node_absolute_position(LiveTL);
            if pos.contains(x, y) {
                return output.workspace_drag_destination(source, x, y);
            }
        }
        None
    }
}

impl NodeBase for DisplayNode {
    fn node_id(&self) -> NodeId {
        self.id
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_display(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let outputs = self.outputs.lock();
        for (_, output) in outputs.deref() {
            visitor.visit_output(output);
        }
        for layer in [
            &self.stacked,
            &self.stacked_above_layers,
            &self.stacked_in_overlay,
        ] {
            for stacked in layer.stacked.iter() {
                stacked.deref().clone().node_visit_dyn(visitor);
            }
        }
    }

    fn node_visible(&self, _tl: TreeTimeline) -> bool {
        true
    }

    fn node_absolute_position(&self, tl: TreeTimeline) -> Rect {
        self.node_state[tl].extents.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        None
    }

    fn node_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        None
    }

    fn node_location(&self) -> Option<NodeLocation> {
        None
    }

    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Display
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        let outputs = self.outputs.lock();
        for output in outputs.values() {
            let pos = output.node_state[LiveTL].pos.get();
            if pos.contains(x, y) {
                let (x, y) = pos.translate(x, y);
                tree.push(FoundNode {
                    node: output.clone(),
                    x,
                    y,
                });
                output.node_find_tree_at(x, y, tree, usecase);
                break;
            }
        }
        FindTreeResult::AcceptsInput
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, _bounds: Option<&Rect>) {
        renderer.render_display(self, x, y);
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("display focus");
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

    fn node_is_display(&self) -> bool {
        true
    }
}

impl NodesStack {
    fn validate(&self, tl: TreeTimeline) {
        if self.visible_valid[tl].replace(true) {
            return;
        }
        for node in self.stacked.iter() {
            node.deref().clone().stacked_validate(tl);
        }
    }

    pub fn iter_visible(&self, tl: TreeTimeline) -> NodeStackVisibleIter {
        self.validate(tl);
        NodeStackVisibleIter {
            tl,
            iter: self.visible[tl].iter(),
        }
    }

    pub fn iter_visible_rev(&self, tl: TreeTimeline) -> RevNodeStackVisibleIter {
        self.validate(tl);
        RevNodeStackVisibleIter {
            tl,
            iter: self.visible[tl].rev_iter(),
        }
    }

    pub fn element(self: &Rc<Self>) -> RefCell<NodesStackElement> {
        RefCell::new(NodesStackElement {
            stack: self.clone(),
            link: Default::default(),
            visible: Default::default(),
        })
    }

    pub fn maybe_has_visible(&self, tl: TreeTimeline) -> bool {
        self.validate(tl);
        self.visible[tl].is_not_empty()
    }

    pub fn definitely_has_no_visible(&self, tl: TreeTimeline) -> bool {
        !self.maybe_has_visible(tl)
    }
}

impl NodesStackElement {
    pub fn clear(&mut self) {
        self.link.take();
        for v in self.visible.values_mut() {
            v.take();
        }
    }

    pub fn restack(&self) {
        if let Some(link) = &self.link {
            self.stack.stacked.add_last_existing(link);
        }
        for tl in TreeTimeline::variants() {
            if let Some(link) = &self.visible[tl]
                && link.node_visible(tl)
            {
                self.stack.visible[tl].add_last_existing(link);
            }
        }
    }

    pub fn restack_on(&mut self, stack: &Rc<NodesStack>) {
        self.stack = stack.clone();
        self.restack();
    }

    pub fn add_last_visible(&mut self, slf: &Rc<impl StackedNode>, tl: TreeTimeline) {
        let link = self.visible[tl].get_or_insert_with(|| LinkedNode::detached(slf.clone()));
        self.stack.visible[tl].add_last_existing(link);
    }

    pub fn invalidate(&self) {
        self.stack.visible_valid[LiveTL].set(false);
        if let Some(v) = &self.link {
            v.deref()
                .deref()
                .clone()
                .stacked_add_stack_op(NodeStackTransactionOp::Invalidate);
        }
    }

    pub fn run_op(&self, op: NodeStackTransactionOp) {
        match op {
            NodeStackTransactionOp::Invalidate => {
                self.stack.visible_valid[RenderTL].set(false);
            }
        }
    }
}

fn next_visible(
    iter: &mut impl Iterator<Item = NodeRef<Rc<dyn StackedNode>>>,
    tl: TreeTimeline,
) -> Option<NodeRef<Rc<dyn StackedNode>>> {
    loop {
        let v = iter.next()?;
        if v.node_visible(tl) {
            return Some(v);
        }
        v.detach();
    }
}

pub struct NodeStackVisibleIter {
    tl: TreeTimeline,
    iter: LinkedListIter<Rc<dyn StackedNode>>,
}

impl Iterator for NodeStackVisibleIter {
    type Item = NodeRef<Rc<dyn StackedNode>>;

    fn next(&mut self) -> Option<Self::Item> {
        next_visible(&mut self.iter, self.tl)
    }
}

pub struct RevNodeStackVisibleIter {
    tl: TreeTimeline,
    iter: RevLinkedListIter<Rc<dyn StackedNode>>,
}

impl Iterator for RevNodeStackVisibleIter {
    type Item = NodeRef<Rc<dyn StackedNode>>;

    fn next(&mut self) -> Option<Self::Item> {
        next_visible(&mut self.iter, self.tl)
    }
}

pub enum DisplayTransactionOp {
    SetExtents(Rect),
}

impl Transactionable for DisplayNode {
    type T = DisplayTransactionOp;

    fn data(&self) -> &TransactionData<Self::T> {
        &self.transaction_data
    }

    fn apply(self: &Rc<Self>, op: Self::T) {
        let s = &self.node_state[RenderTL];
        match op {
            DisplayTransactionOp::SetExtents(v) => {
                s.extents.set(v);
            }
        }
    }
}

pub enum NodeStackTransactionOp {
    Invalidate,
}
