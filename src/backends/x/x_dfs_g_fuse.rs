/*
dir backend {
    @inherit dfs_backend,
    render_device: reg,
    drm_dev: reg,
    root: reg,
    cursor: reg,
    num_outputs: reg,
    num_seats: reg,
    num_mouse_seats: reg,
    outputs: view (key = 0),
    seats: view (key = 0),
}

dir output {
    id: reg,
    window: reg,
    width: reg,
    height: reg,
    serial: reg,
    next_msc: reg,
    next_image: reg,
    has_cb: reg,
    state_serial: reg,
    enabled: reg,
    active: reg,
    mode_width: reg,
    mode_height: reg,
    refresh_rate_millihz: reg,
    non_desktop_override: reg,
    vrr: reg,
    tearing: reg,
    format: reg,
}

dir seat {
    kb_id: reg,
    mouse_id: reg,
    kb_name: reg,
    mouse_name: reg,
    kb: reg,
    mouse: reg,
    removed: reg,
    num_kb_events: reg,
    num_mouse_events: reg,
    num_buttons_mapped: reg,
}
 */
use crate::backends::x::XBackend;
use crate::backends::x::XOutput;
use crate::backends::x::XSeat;
use crate::backends::x::x_dfs_g_fuse::generated::backend;
use crate::backends::x::x_dfs_g_fuse::generated::output;
use crate::backends::x::x_dfs_g_fuse::generated::seat;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::fuse::fuse_globals::dfs_backend;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::CopyHashMapDir;
use crate::utils::fuse::fuse_views::CopyHashMapDirView;
use crate::utils::major_minor::major_minor;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;
use std::str::FromStr;

impl XBackend {
    pub(super) fn debugfs(self: Rc<Self>) -> Option<FuseInodeWithKey> {
        Some(self.tv_wrap_rc::<backend::View>().without_key())
    }
}

impl dfs_backend::Dir for XBackend {
    fn read_backend_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        "x11".str_fmt(buf, ctx);
    }
}

impl backend::Dir for XBackend {
    type ViewOutputs = CopyHashMapDir<Outputs>;
    type ViewSeats = CopyHashMapDir<Seats>;

    fn read_render_device(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_device_id.raw().str_fmt(buf, ctx);
    }

    fn read_drm_dev(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        major_minor(self.drm_dev).str_fmt(buf, ctx);
    }

    fn read_root(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.root.str_fmt(buf, ctx);
    }

    fn read_cursor(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor.str_fmt(buf, ctx);
    }

    fn read_num_outputs(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.outputs.len().str_fmt(buf, ctx);
    }

    fn read_num_seats(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.seats.len().str_fmt(buf, ctx);
    }

    fn read_num_mouse_seats(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mouse_seats.len().str_fmt(buf, ctx);
    }
}

struct Outputs;

impl CopyHashMapDirView<XBackend> for Outputs {
    type Key = u32;
    type Value = XOutput;
    type View = output::View;
    type StringBuf = itoa::Buffer;

    fn get(t: &XBackend, _key: u64) -> &CopyHashMap<Self::Key, Rc<Self::Value>> {
        &t.outputs
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(*key)
    }

    fn parse_name(key: &str) -> Option<Self::Key> {
        u32::from_str(key).ok()
    }
}

struct Seats;

impl CopyHashMapDirView<XBackend> for Seats {
    type Key = u16;
    type Value = XSeat;
    type View = seat::View;
    type StringBuf = itoa::Buffer;

    fn get(t: &XBackend, _key: u64) -> &CopyHashMap<Self::Key, Rc<Self::Value>> {
        &t.seats
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(*key)
    }

    fn parse_name(key: &str) -> Option<Self::Key> {
        u16::from_str(key).ok()
    }
}

impl output::Dir for XOutput {
    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_window(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.window.str_fmt(buf, ctx);
    }

    fn read_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.width.get().str_fmt(buf, ctx);
    }

    fn read_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.height.get().str_fmt(buf, ctx);
    }

    fn read_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.serial.get().str_fmt(buf, ctx);
    }

    fn read_next_msc(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.next_msc.get().str_fmt(buf, ctx);
    }

    fn read_next_image(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.next_image.get().str_fmt(buf, ctx);
    }

    fn read_has_cb(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cb.get().is_some().str_fmt(buf, ctx);
    }

    fn read_state_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().serial.raw().str_fmt(buf, ctx);
    }

    fn read_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().enabled.str_fmt(buf, ctx);
    }

    fn read_active(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().active.str_fmt(buf, ctx);
    }

    fn read_mode_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().mode.width.str_fmt(buf, ctx);
    }

    fn read_mode_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().mode.height.str_fmt(buf, ctx);
    }

    fn read_refresh_rate_millihz(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state
            .borrow()
            .mode
            .refresh_rate_millihz
            .str_fmt(buf, ctx);
    }

    fn read_non_desktop_override(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().non_desktop_override.str_fmt(buf, ctx);
    }

    fn read_vrr(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().vrr.str_fmt(buf, ctx);
    }

    fn read_tearing(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().tearing.str_fmt(buf, ctx);
    }

    fn read_format(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.state.borrow().format.name.str_fmt(buf, ctx);
    }
}

impl seat::Dir for XSeat {
    fn read_kb_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.kb_id.raw().str_fmt(buf, ctx);
    }

    fn read_mouse_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mouse_id.raw().str_fmt(buf, ctx);
    }

    fn read_kb_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.kb_name.str_fmt(buf, ctx);
    }

    fn read_mouse_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mouse_name.str_fmt(buf, ctx);
    }

    fn read_kb(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.kb.str_fmt(buf, ctx);
    }

    fn read_mouse(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mouse.str_fmt(buf, ctx);
    }

    fn read_removed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.removed.get().str_fmt(buf, ctx);
    }

    fn read_num_kb_events(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.kb_events.borrow().len().str_fmt(buf, ctx);
    }

    fn read_num_mouse_events(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mouse_events.borrow().len().str_fmt(buf, ctx);
    }

    fn read_num_buttons_mapped(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.button_map.len().str_fmt(buf, ctx);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_0cace17ed56a56914aaab94404618806fdd1fe9bf88de6edbbc5f07322930da0.rs",
));
// FUSE GENERATED STOP
