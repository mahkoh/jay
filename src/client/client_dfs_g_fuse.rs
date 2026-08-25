/*
dir client {
    id: reg,
    checking_queue_size: reg,
    socket: reg,
    is_xwayland: reg,
    last_enter_serial: reg,
    symmetric_delete: reg,
    last_xwayland_serial: reg,
    wire_scale: reg,
    focus_stealing_serial: reg,
    num_live_sessions: reg,
    connect_time_us: reg,
    terminate_shutdown: reg,
    terminate_kill: reg,
    objects: view (key = 0),
    pidinfo: view (key = 0),
}

dir pidinfo {
    uid: reg,
    pid: reg,
    comm: reg,
    exe: reg,
}
 */
use crate::client::Client;
use crate::client::client_dfs_g_fuse::generated::client;
use crate::client::client_dfs_g_fuse::generated::pidinfo;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::IterDirDyn;
use crate::utils::fuse::fuse_views::IterDirDynView;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::str_fmt::StrFmtUs;
use crate::wire::ObjectId;
pub use client::View as ClientView;
use std::rc::Rc;
use std::str::FromStr;

impl client::Dir for Client {
    type ViewObjects = IterDirDyn<Objects>;
    type ViewPidinfo = pidinfo::View;

    fn read_id(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.id.0.str_fmt(dst, ctx);
    }

    fn read_checking_queue_size(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.checking_queue_size.get().str_fmt(dst, ctx);
    }

    fn read_socket(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.socket.raw().str_fmt(dst, ctx);
    }

    fn read_is_xwayland(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.is_xwayland.str_fmt(dst, ctx);
    }

    fn read_last_enter_serial(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.last_enter_serial.get().str_fmt(dst, ctx);
    }

    fn read_symmetric_delete(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.symmetric_delete.get().str_fmt(dst, ctx);
    }

    fn read_last_xwayland_serial(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.last_xwayland_serial.get().str_fmt(dst, ctx);
    }

    fn read_wire_scale(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.wire_scale.get().str_fmt(dst, ctx);
    }

    fn read_focus_stealing_serial(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.focus_stealing_serial.get().str_fmt(dst, ctx);
    }

    fn read_num_live_sessions(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.num_live_sessions.get().str_fmt(dst, ctx);
    }

    fn read_connect_time_us(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        StrFmtUs(self.connect_time_us).str_fmt(dst, ctx);
    }

    fn read_terminate_shutdown(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.terminate_shutdown.get().str_fmt(dst, ctx);
    }

    fn read_terminate_kill(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        self.terminate_kill.get().str_fmt(dst, ctx);
    }
}

struct Objects;

impl IterDirDynView<Client> for Objects {
    fn iter(t: &Rc<Client>, _key: u64, mut f: impl FnMut(&str, FuseInodeWithKey)) {
        let mut buf = itoa::Buffer::new();
        for (id, obj) in t.objects.registry.lock().iter() {
            f(buf.format(id.raw()), obj.clone().object_debugfs(&t));
        }
    }

    fn get(t: &Rc<Client>, _key: u64, name: &str) -> Option<FuseInodeWithKey> {
        let id = u64::from_str(name).ok()?;
        let obj = t.objects.get_obj(ObjectId::from_raw(id)).ok()?;
        Some(obj.object_debugfs(t))
    }
}

impl pidinfo::Dir for Client {
    fn read_uid(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pid_info.uid.str_fmt(buf, ctx);
    }

    fn read_pid(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pid_info.pid.str_fmt(buf, ctx);
    }

    fn read_comm(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pid_info.comm.str_fmt(buf, ctx);
    }

    fn read_exe(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pid_info.exe.str_fmt(buf, ctx);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_89862741d256dc03cf1e1033b7def559aa102c9faf385effd27c2da590e7bd87.rs",
));
// FUSE GENERATED STOP
