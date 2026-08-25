/*
dir dfs_backend (abstract, global) {
    @inherit dfs_backend_common,
    backend_name: reg,
}

dir dfs_backend_common (abstract, global) {
    backend_import_environment: reg,
    backend_supports_presentation_feedback: reg,
}
 */
use crate::backend::Backend;
use crate::utils::fuse::fuse_globals::dfs_backend_common;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;

impl<T> dfs_backend_common::Dir for T
where
    T: Backend,
{
    fn read_backend_import_environment(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.import_environment().str_fmt(buf, ctx);
    }

    fn read_backend_supports_presentation_feedback(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.supports_presentation_feedback().str_fmt(buf, ctx);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_d7a15b8285c664f13fe7c3526acd08119bd6651177a13dd6e3ae8f0323d08dc3.rs",
));
// FUSE GENERATED STOP
