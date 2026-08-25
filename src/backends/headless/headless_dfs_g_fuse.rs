/*
dir backend {
    @inherit dfs_backend,
    monitor_fd: reg,
    render_device: reg,
    devs: view (key = 0),
}

dir dev {
    id: reg,
    dev: reg,
    api: reg,
}
 */
use crate::backends::headless::HeadlessBackend;
use crate::backends::headless::HeadlessDrmDevice;
use crate::backends::headless::headless_dfs_g_fuse::generated::backend;
use crate::backends::headless::headless_dfs_g_fuse::generated::dev;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::fuse::fuse_globals::dfs_backend;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::DevTDir;
use crate::utils::fuse::fuse_views::DevTDirView;
use crate::utils::major_minor::major_minor;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;
use uapi::c::dev_t;

impl HeadlessBackend {
    pub(super) fn debugfs(self: Rc<Self>) -> Option<FuseInodeWithKey> {
        Some(self.tv_wrap_rc::<backend::View>().without_key())
    }
}

impl dfs_backend::Dir for HeadlessBackend {
    fn read_backend_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        "headless".str_fmt(buf, ctx);
    }
}

impl backend::Dir for HeadlessBackend {
    type ViewDevs = DevTDir<DrmDevs>;

    fn read_monitor_fd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.monitor_fd.raw().str_fmt(buf, ctx);
    }

    fn read_render_device(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.render_device.get().map(|v| v.raw()).str_fmt(buf, ctx);
    }
}

struct DrmDevs;

impl DevTDirView<HeadlessBackend> for DrmDevs {
    type D = HeadlessDrmDevice;
    type V = dev::View;

    fn devs(t: &HeadlessBackend) -> &CopyHashMap<dev_t, Rc<Self::D>> {
        &t.devs
    }
}

impl dev::Dir for HeadlessDrmDevice {
    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_dev(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        major_minor(self.dev).str_fmt(buf, ctx);
    }

    fn read_api(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.api.get().to_str().str_fmt(buf, ctx);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_7d0cc59f634493c06b4e5a8e1f6a421fe46788b18710017e0bbb0782a20d549a.rs",
));
// FUSE GENERATED STOP
