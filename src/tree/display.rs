use {
    crate::{
        backend::ConnectorId,
        cursor::KnownCursor,
        ifs::wl_seat::{NodeSeatState, WlSeatGlobal},
        rect::Rect,
        renderer::Renderer,
        tree::{
            walker::NodeVisitor, FindTreeResult, FoundNode, Node, NodeId, OutputNode, StackedNode,
        },
        utils::{copyhashmap::CopyHashMap, linkedlist::LinkedList},
    },
    std::{cell::Cell, ops::Deref, rc::Rc},
};

pub struct DisplayNode {
    pub id: NodeId,
    pub extents: Cell<Rect>,
    pub outputs: CopyHashMap<ConnectorId, Rc<OutputNode>>,
    pub stacked: LinkedList<Rc<dyn StackedNode>>,
    pub seat_state: NodeSeatState,
}

impl DisplayNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            extents: Default::default(),
            outputs: Default::default(),
            stacked: Default::default(),
            seat_state: Default::default(),
        }
    }

    pub fn clear(&self) {
        self.outputs.clear();
        self.seat_state.clear();
    }

    pub fn update_extents(&self) {
        let outputs = self.outputs.lock();
        let mut x1 = i32::MAX;
        let mut y1 = i32::MAX;
        let mut x2 = i32::MIN;
        let mut y2 = i32::MIN;
        for output in outputs.values() {
            let pos = output.global.pos.get();
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
        self.extents.set(Rect::new(x1, y1, x2, y2).unwrap());
    }
}

impl Node for DisplayNode {
    fn node_id(&self) -> NodeId {
        self.id
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_display(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let outputs = self.outputs.lock();
        for (_, output) in outputs.deref() {
            visitor.visit_output(output);
        }
        for stacked in self.stacked.iter() {
            stacked.deref().clone().node_visit(visitor);
        }
    }

    fn node_visible(&self) -> bool {
        true
    }

    fn node_absolute_position(&self) -> Rect {
        self.extents.get()
    }

    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let outputs = self.outputs.lock();
        for output in outputs.values() {
            let pos = output.global.pos.get();
            if pos.contains(x, y) {
                let (x, y) = pos.translate(x, y);
                tree.push(FoundNode {
                    node: output.clone(),
                    x,
                    y,
                });
                output.node_find_tree_at(x, y, tree);
                break;
            }
        }
        FindTreeResult::AcceptsInput
    }

    fn node_render(
        &self,
        renderer: &mut Renderer,
        x: i32,
        y: i32,
        _max_width: i32,
        _max_height: i32,
    ) {
        renderer.render_display(self, x, y);
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("display focus");
        seat.set_known_cursor(KnownCursor::Default);
    }
}
