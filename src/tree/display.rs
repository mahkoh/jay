use {
    crate::{
        backend::ConnectorId,
        cursor::KnownCursor,
        ifs::{
            wl_seat::{NodeSeatState, WlSeatGlobal},
        },
        rect::Rect,
        render::Renderer,
        tree::{
            walker::NodeVisitor, FindTreeResult, FoundNode, Node, NodeId, OutputNode, SizedNode,
        },
        utils::{copyhashmap::CopyHashMap, linkedlist::LinkedList},
    },
    std::{cell::Cell, ops::Deref, rc::Rc},
};

pub struct DisplayNode {
    pub id: NodeId,
    pub extents: Cell<Rect>,
    pub outputs: CopyHashMap<ConnectorId, Rc<OutputNode>>,
    pub stacked: LinkedList<Rc<dyn Node>>,
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

impl SizedNode for DisplayNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        let mut outputs = self.outputs.lock();
        for output in outputs.values() {
            output.node_destroy(false);
        }
        outputs.clear();
        for stacked in self.stacked.iter() {
            stacked.node_destroy(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_display(self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let outputs = self.outputs.lock();
        for (_, output) in outputs.deref() {
            visitor.visit_output(output);
        }
        for stacked in self.stacked.iter() {
            stacked.deref().clone().node_visit(visitor);
        }
    }

    fn visible(&self) -> bool {
        true
    }

    fn parent(&self) -> Option<Rc<dyn Node>> {
        None
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
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
                output.find_tree_at(x, y, tree);
                break;
            }
        }
        FindTreeResult::AcceptsInput
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_display(self, x, y);
    }
}
