use crate::tree::ToplevelNode;

pub trait TestToplevelNodeExt {
    fn center(&self) -> (i32, i32);
}

impl TestToplevelNodeExt for dyn ToplevelNode {
    fn center(&self) -> (i32, i32) {
        self.node_mapped_position().center()
    }
}
