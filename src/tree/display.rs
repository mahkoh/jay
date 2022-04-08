use {
    crate::{
        backend::ConnectorId,
        cursor::KnownCursor,
        ifs::{
            wl_seat::{NodeSeatState, WlSeatGlobal},
            zwlr_layer_shell_v1::{OVERLAY, TOP},
        },
        tree::{walker::NodeVisitor, FindTreeResult, FoundNode, Node, NodeId, OutputNode},
        utils::{copyhashmap::CopyHashMap, linkedlist::LinkedList},
    },
    std::{ops::Deref, rc::Rc},
};

pub struct DisplayNode {
    pub id: NodeId,
    pub outputs: CopyHashMap<ConnectorId, Rc<OutputNode>>,
    pub stacked: LinkedList<Rc<dyn Node>>,
    pub seat_state: NodeSeatState,
}

impl DisplayNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            outputs: Default::default(),
            stacked: Default::default(),
            seat_state: Default::default(),
        }
    }
}

impl Node for DisplayNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn visible(&self) -> bool {
        true
    }

    fn destroy_node(&self, _detach: bool) {
        let mut outputs = self.outputs.lock();
        for output in outputs.values() {
            output.destroy_node(false);
        }
        outputs.clear();
        for stacked in self.stacked.iter() {
            stacked.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_display(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let outputs = self.outputs.lock();
        for (_, output) in outputs.deref() {
            visitor.visit_output(output);
        }
        for stacked in self.stacked.iter() {
            stacked.deref().clone().visit(visitor);
        }
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
                if output.find_layer_surface_at(x, y, &[OVERLAY, TOP], tree)
                    == FindTreeResult::AcceptsInput
                {
                    return FindTreeResult::AcceptsInput;
                }
                tree.pop();
                break;
            }
        }
        for stacked in self.stacked.rev_iter() {
            let ext = stacked.absolute_position();
            if stacked.absolute_position_constrains_input() && !ext.contains(x, y) {
                // TODO: make constrain always true
                continue;
            }
            let (x, y) = ext.translate(x, y);
            let idx = tree.len();
            tree.push(FoundNode {
                node: stacked.deref().clone(),
                x,
                y,
            });
            match stacked.find_tree_at(x, y, tree) {
                FindTreeResult::AcceptsInput => {
                    return FindTreeResult::AcceptsInput;
                }
                FindTreeResult::Other => {
                    tree.drain(idx..);
                }
            }
        }
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
}
