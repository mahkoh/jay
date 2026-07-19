use crate::tree::ToplevelNode;
use crate::tree::TreeTimeline::LiveTL;

pub trait TestToplevelNodeExt {
    fn center(&self) -> (i32, i32);
}

impl TestToplevelNodeExt for dyn ToplevelNode {
    fn center(&self) -> (i32, i32) {
        self.node_absolute_position(LiveTL).center()
    }
}
