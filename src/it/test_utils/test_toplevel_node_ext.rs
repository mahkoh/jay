use crate::{it::test_utils::test_rect_ext::TestRectExt, tree::ToplevelNode};

pub trait TestToplevelNodeExt {
    fn center(&self) -> (i32, i32);
}

impl TestToplevelNodeExt for dyn ToplevelNode {
    fn center(&self) -> (i32, i32) {
        self.node_absolute_position().center()
    }
}
