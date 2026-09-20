/*

*/
use crate::tree::Node;
use crate::utils::fuse::fuse_globals::dfs_node_seat_state;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;

impl<T> dfs_node_seat_state::Dir for T
where
    T: Node,
{
    fn read_no_focus_history(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_seat_state()
            .no_focus_history
            .get()
            .str_fmt(buf, ctx);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_f51f1574d6aec13869c734f2e12cee81f37ae1e90f8fb014a1f73c71b6e5b942.rs",
));
// FUSE GENERATED STOP
