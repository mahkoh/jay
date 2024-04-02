use crate::tree::ToplevelNode;

pub trait TestToplevelNodeExt {
    fn center(&self) -> (i32, i32);
}

impl TestToplevelNodeExt for dyn ToplevelNode {
    fn center(&self) -> (i32, i32) {
        let rect = self.node_absolute_position();
        ((rect.x1() + rect.x2()) / 2, (rect.y1() + rect.y2()) / 2)
    }
}
