/*
dir root {
    version: reg,
}
*/
use crate::state::State;
use crate::state::state_dfs_g_fuse::generated::root;
use crate::utils::fuse::fuse_error::FuseError;
use crate::utils::fuse::fuse_inode::FuseInode;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use crate::version::VERSION;
use std::rc::Rc;
use uapi::OwnedFd;

impl State {
    pub fn debugfs(self: &Rc<Self>) -> Rc<dyn FuseInode> {
        self.tv_wrap_rc_ref_clone::<root::View>()
    }

    pub fn snapshot_debugfs(
        self: &Rc<Self>,
        root: &str,
        json: bool,
    ) -> Result<Rc<OwnedFd>, FuseError> {
        let inode: Rc<dyn FuseInode> = self.tv_wrap_rc_ref_clone::<root::View>();
        inode.snapshot(0, root, json)
    }
}

impl root::Dir for State {
    fn read_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        VERSION.str_fmt(buf, ctx)
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_8563e4e637c55ed8bf14a6455be558a3271e5c626f37595a04b66b07ff35056e.rs",
));
// FUSE GENERATED STOP
