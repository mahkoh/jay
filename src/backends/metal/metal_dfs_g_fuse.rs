/*
dir backend {
    @inherit dfs_backend,
    monitor_fd: reg,
    libinput_fd: reg,
    signaled_sync_file: reg (opt, no_timeout),
    render_device: link (opt),
    render_ctx: view (opt, other, key = 0),
    devs: view (key = 0),
}

dir render_ctx {
    dev: link,
    dev_id: reg,
    devnode: reg,
    copy_device_id: reg,
    copy_device_devnum: reg,
    has_copy_device: reg,
}

dir dev {
    id: reg,
    dev: reg,
    devnode: reg,
    master_fd: reg,
    is_render_device: reg,
    supports_kms: reg,
    supports_async_commit: reg,
    supports_plane_color_pipelines: reg,
    use_plane_color_pipelines: reg,
    direct_scanout_enabled: reg,
    paused: reg,
    min_post_commit_margin: reg,
    cursor_width: reg,
    cursor_height: reg,
    is_nvidia: reg,
    is_amd: reg,
    connectors: view (key = 0),
    crtcs: view (key = 0),
    encoders: view (key = 0),
    planes: view (key = 0),
}

dir connector {
    id: reg,
    kernel_id: reg,
    connector_id: reg,
    buffers_idle: reg,
    crtc_idle: reg,
    has_damage: reg,
    cursor_changed: reg,
    cursor_damage: reg,
    cursor_enabled: reg,
    cursor_x: reg,
    cursor_y: reg,
    next_vblank_nsec: reg,
    frontend_state: reg,
    frontend_non_desktop: reg,
    lease: reg,
    has_buffers: reg,
    has_cursor_buffers: reg,
    has_cursor_sync: reg,
    cursor_swap_buffer: reg,
    version: reg,
    expected_sequence: reg,
    pre_commit_margin: reg,
    pre_commit_margin_decay: reg,
    post_commit_margin: reg,
    post_commit_margin_decay: reg,
    vblank_miss_sec: reg,
    vblank_miss_this_sec: reg,
    presentation_is_sync: reg,
    presentation_is_zero_copy: reg,
    direct_scanout_active: reg,
    last_direct_scanout_error: reg,
    active_framebuffer: view (opt, key = 0),
    has_gamma_lut: reg,
    color_description: reg,
    fb_color_description: reg,
    fb_render_intent: reg,
    connection: reg,
    mm_width: reg,
    mm_height: reg,
    refresh: reg,
    non_desktop: reg,
    non_desktop_effective: reg,
    vrr_capable: reg,
    supports_bt2020: reg,
    supports_pq: reg,
    mode_width: reg,
    mode_height: reg,
    mode_refresh_rate_millihz: reg,
    drm_locked: reg,
    drm_fb: reg,
    drm_fb_idx: reg,
    drm_cursor_fb: reg,
    drm_cursor_fb_idx: reg,
    drm_cursor_x: reg,
    drm_cursor_y: reg,
    drm_src_w: reg,
    drm_src_h: reg,
    drm_crtc_x: reg,
    drm_crtc_y: reg,
    drm_crtc_w: reg,
    drm_crtc_h: reg,
    drm_has_out_fd: reg,
    drm_link_status: reg,
    drm_crtc_id: reg,
    drm_color_space: reg,
    drm_hdr_metadata_blob_id: reg,
    crtc: link (opt),
    primary_plane: link (opt),
    cursor_plane: link (opt),
    possible_crtcs: view (key = 0),
}

dir crtc {
    id: reg,
    idx: reg,
    master_fd: reg,
    lease: reg,
    out_fence_ptr: reg,
    gamma_lut_size: reg,
    sequence: reg,
    have_queued_sequence: reg,
    needs_vblank_emulation: reg,
    assigned_connector: reg,
    active: reg,
    mode_blob_id: reg,
    vrr_enabled: reg,
    has_mode_blob: reg,
    mode_name: reg,
    mode_clock: reg,
    mode_hdisplay: reg,
    mode_vdisplay: reg,
    mode_vrefresh: reg,
    mode_flags: reg,
    mode_type: reg,
    connector: link (opt),
    pending_flip: link (opt),
    possible_planes: view (key = 0),
}

dir encoder {
    id: reg,
    crtcs: view (key = 0),
}

dir plane {
    id: reg,
    ty: reg,
    master_fd: reg,
    possible_crtcs: reg,
    lease: reg,
    mode_w: reg,
    mode_h: reg,
    in_fence_fd: reg,
    formats: reg,
    assigned_crtc: link (opt),
    has_buffers: reg,
    fb_id: reg,
    src_x: reg,
    src_y: reg,
    src_w: reg,
    src_h: reg,
    crtc: link (opt),
    crtc_x: reg,
    crtc_y: reg,
    crtc_w: reg,
    crtc_h: reg,
}

dir framebuffer {
    fb: reg,
    fb_cd: reg,
    size: reg,
    format: reg,
    dmabuf: reg (opt),
    locked: reg,
    direct_scanout_data: view (opt, key = 0),
}

dir direct_scanout_data {
    size: reg,
    format: reg,
    dmabuf: reg (opt),
    has_tex_resv: reg,
    acquire_sync: reg,
    release_sync: reg,
    has_fb_resv: reg,
    is_lazy: reg,
    fb: reg,
    position: reg,
}
 */
use crate::backends::metal::MetalBackend;
use crate::backends::metal::metal_dfs_g_fuse::generated::backend;
use crate::backends::metal::metal_dfs_g_fuse::generated::connector;
use crate::backends::metal::metal_dfs_g_fuse::generated::crtc;
use crate::backends::metal::metal_dfs_g_fuse::generated::dev;
use crate::backends::metal::metal_dfs_g_fuse::generated::direct_scanout_data;
use crate::backends::metal::metal_dfs_g_fuse::generated::encoder;
use crate::backends::metal::metal_dfs_g_fuse::generated::framebuffer;
use crate::backends::metal::metal_dfs_g_fuse::generated::plane;
use crate::backends::metal::metal_dfs_g_fuse::generated::render_ctx;
use crate::backends::metal::video::FrontState;
use crate::backends::metal::video::MetalConnector;
use crate::backends::metal::video::MetalCrtc;
use crate::backends::metal::video::MetalDrmDeviceData;
use crate::backends::metal::video::MetalEncoder;
use crate::backends::metal::video::MetalPlane;
use crate::backends::metal::video::MetalRenderContext;
use crate::dfs::dfs_helpers::format_path_link;
use crate::dfs::dfs_helpers::write_root_link;
use crate::utils::binary_search_map::BinarySearchMap;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fuse::fuse_globals::dfs_backend;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::BinarySearchMapDir;
use crate::utils::fuse::fuse_views::BinarySearchMapDirView;
use crate::utils::fuse::fuse_views::CopyHashMapDir;
use crate::utils::fuse::fuse_views::CopyHashMapDirView;
use crate::utils::fuse::fuse_views::DevTDir;
use crate::utils::fuse::fuse_views::DevTDirView;
use crate::utils::fuse::fuse_views::FuseLink;
use crate::utils::fuse::fuse_views::FuseLinkView;
use crate::utils::fuse::fuse_views::HashMapDir;
use crate::utils::fuse::fuse_views::HashMapDirView;
use crate::utils::get_inner::GetInner;
use crate::utils::major_minor::MajorMinor;
use crate::utils::major_minor::major_minor;
use crate::utils::static_text::StaticText;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use crate::video::drm::ConnectorStatus;
use crate::video::drm::DrmConnector;
use crate::video::drm::DrmCrtc;
use crate::video::drm::DrmEncoder;
use crate::video::drm::DrmObject;
use crate::video::drm::DrmPlane;
use arrayvec::ArrayString;
use bstr::ByteSlice;
use hashbrown::HashMap;
use jay_proc::StrFmt;
use std::cell::Ref;
use std::rc::Rc;
use std::str::FromStr;
use uapi::c::dev_t;

fn write_dev_link(buf: &mut String, up: u64, devnum: dev_t) {
    let MajorMinor { major, minor } = major_minor(devnum);
    let mut name = ArrayString::<41>::new();
    let mut itoa = itoa::Buffer::new();
    name.push_str(itoa.format(major));
    name.push_str(":");
    name.push_str(itoa.format(minor));
    format_path_link(buf, up, "backend/devs", &name);
}

struct ObjLink;

fn write_obj_link(buf: &mut String, depth: u64, dev: dev_t, category: &str, id: u32) {
    write_obj_link2(buf, depth, dev, category, id, false);
}

fn write_obj_link2(
    buf: &mut String,
    depth: u64,
    dev: dev_t,
    category: &str,
    id: u32,
    external: bool,
) {
    let MajorMinor { major, minor } = major_minor(dev);
    let mut tmp = itoa::Buffer::new();
    if external {
        write_root_link(buf, depth);
        buf.push_str("backend/");
    } else {
        for _ in 2..depth {
            buf.push_str("../");
        }
    }
    buf.push_str("devs/");
    buf.push_str(tmp.format(major));
    buf.push_str(":");
    buf.push_str(tmp.format(minor));
    buf.push_str("/");
    buf.push_str(category);
    buf.push_str("/");
    buf.push_str(tmp.format(id));
}

const CONNECTORS: &str = "connectors";
const CRTCS: &str = "crtcs";
const PLANES: &str = "planes";

impl FuseLinkView<MetalConnector> for ObjLink {
    fn readlink(t: &MetalConnector, _key: u64, depth: u64, buf: &mut String) {
        write_obj_link(buf, depth, t.dev.devnum, CONNECTORS, t.id.0);
    }
}

impl FuseLinkView<MetalCrtc> for ObjLink {
    fn readlink(t: &MetalCrtc, _key: u64, depth: u64, buf: &mut String) {
        write_obj_link(buf, depth, t.master.dev(), CRTCS, t.id.0);
    }
}

impl FuseLinkView<MetalPlane> for ObjLink {
    fn readlink(t: &MetalPlane, _key: u64, depth: u64, buf: &mut String) {
        write_obj_link(buf, depth, t.master.dev(), PLANES, t.id.0);
    }
}

impl MetalBackend {
    pub(super) fn debugfs(self: Rc<Self>) -> Option<FuseInodeWithKey> {
        Some(self.tv_wrap_rc::<backend::View>().without_key())
    }
}

impl dfs_backend::Dir for MetalBackend {
    fn read_backend_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        "metal".str_fmt(buf, ctx);
    }
}

impl backend::Dir for MetalBackend {
    type ViewRenderCtx = render_ctx::View;
    type BaseRenderCtx = MetalRenderContext;
    type ViewDevs = DevTDir<DrmDevs>;

    fn read_monitor_fd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.monitor_fd.raw().str_fmt(buf, ctx);
    }

    fn read_libinput_fd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.libinput_fd.raw().str_fmt(buf, ctx);
    }

    fn has_signaled_sync_file(&self) -> bool {
        self.signaled_sync_file.is_some()
    }

    fn read_signaled_sync_file(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(sf) = self.signaled_sync_file.get() {
            sf.raw().str_fmt(buf, ctx);
        }
    }

    fn has_render_device(&self) -> bool {
        self.ctx.is_some()
    }

    fn readlink_render_device(&self, depth: u64, buf: &mut String) {
        if let Some(ctx) = self.ctx.get() {
            write_dev_link(buf, depth, ctx.gbm.drm.dev());
        }
    }

    fn get_render_ctx(self: &Rc<Self>, _key: u64) -> Option<Rc<MetalRenderContext>> {
        self.ctx.get()
    }
}

impl render_ctx::Dir for MetalRenderContext {
    fn readlink_dev(&self, depth: u64, buf: &mut String) {
        write_dev_link(buf, depth, self.gbm.drm.dev());
    }

    fn read_dev_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev_id.raw().str_fmt(buf, ctx);
    }

    fn read_devnode(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.devnode.as_bytes().as_bstr().str_fmt(buf, ctx);
    }

    fn read_copy_device_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.copy_device.id.raw().str_fmt(buf, ctx);
    }

    fn read_copy_device_devnum(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        major_minor(self.copy_device.devnum).str_fmt(buf, ctx);
    }

    fn read_has_copy_device(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.copy_device
            .dev
            .get()
            .map(|d| d.is_some())
            .str_fmt(buf, ctx);
    }
}

struct DrmDevs;

impl DevTDirView<MetalBackend> for DrmDevs {
    type D = MetalDrmDeviceData;
    type V = dev::View;

    fn devs(t: &MetalBackend) -> &CopyHashMap<dev_t, Rc<Self::D>> {
        &t.device_holder.drm_devices
    }
}

impl dev::Dir for MetalDrmDeviceData {
    type ViewConnectors = CopyHashMapDir<Connectors>;
    type ViewCrtcs = HashMapDir<Crtcs>;
    type ViewEncoders = HashMapDir<Encoders>;
    type ViewPlanes = HashMapDir<Planes>;

    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.id.raw().str_fmt(buf, ctx);
    }

    fn read_dev(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        major_minor(self.dev.devnum).str_fmt(buf, ctx);
    }

    fn read_devnode(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.devnode.as_bytes().as_bstr().str_fmt(buf, ctx);
    }

    fn read_master_fd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.master.raw().str_fmt(buf, ctx);
    }

    fn read_is_render_device(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.is_render_device().str_fmt(buf, ctx);
    }

    fn read_supports_kms(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.supports_kms.str_fmt(buf, ctx);
    }

    fn read_supports_async_commit(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.supports_async_commit.str_fmt(buf, ctx);
    }

    fn read_supports_plane_color_pipelines(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.supports_plane_color_pipelines.str_fmt(buf, ctx);
    }

    fn read_use_plane_color_pipelines(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.use_plane_color_pipelines.get().str_fmt(buf, ctx);
    }

    fn read_direct_scanout_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.direct_scanout_enabled.get().str_fmt(buf, ctx);
    }

    fn read_paused(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.paused.get().str_fmt(buf, ctx);
    }

    fn read_min_post_commit_margin(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.min_post_commit_margin.get().str_fmt(buf, ctx);
    }

    fn read_cursor_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.cursor_width.str_fmt(buf, ctx);
    }

    fn read_cursor_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.cursor_height.str_fmt(buf, ctx);
    }

    fn read_is_nvidia(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.vendor.is_nvidia.str_fmt(buf, ctx);
    }

    fn read_is_amd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dev.vendor.is_amd.str_fmt(buf, ctx);
    }
}

struct Connectors;

impl CopyHashMapDirView<MetalDrmDeviceData> for Connectors {
    type Key = DrmConnector;
    type Value = MetalConnector;
    type View = connector::View;
    type StringBuf = itoa::Buffer;

    fn get(t: &MetalDrmDeviceData, _key: u64) -> &CopyHashMap<Self::Key, Rc<Self::Value>> {
        &t.connectors
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(key.0)
    }

    fn parse_name(key: &str) -> Option<Self::Key> {
        u32::from_str(key).ok().map(DrmConnector)
    }
}

struct Crtcs;

impl HashMapDirView<MetalDrmDeviceData> for Crtcs {
    type BuildHasher = ahash::RandomState;
    type Key = DrmCrtc;
    type Value = MetalCrtc;
    type View = crtc::View;
    type StringBuf = itoa::Buffer;

    fn get(
        t: &MetalDrmDeviceData,
        _key: u64,
    ) -> impl GetInner<HashMap<Self::Key, Rc<Self::Value>, Self::BuildHasher>> {
        &t.dev.crtcs
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(key.0)
    }

    fn parse_name(name: &str) -> Option<Self::Key> {
        u32::from_str(name).ok().map(DrmCrtc)
    }
}

struct Encoders;

impl HashMapDirView<MetalDrmDeviceData> for Encoders {
    type BuildHasher = ahash::RandomState;
    type Key = DrmEncoder;
    type Value = MetalEncoder;
    type View = encoder::View;
    type StringBuf = itoa::Buffer;

    fn get(
        t: &MetalDrmDeviceData,
        _key: u64,
    ) -> impl GetInner<HashMap<Self::Key, Rc<Self::Value>, Self::BuildHasher>> {
        &t.dev.encoders
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(key.0)
    }

    fn parse_name(name: &str) -> Option<Self::Key> {
        u32::from_str(name).ok().map(DrmEncoder)
    }
}

struct Planes;

impl HashMapDirView<MetalDrmDeviceData> for Planes {
    type BuildHasher = ahash::RandomState;
    type Key = DrmPlane;
    type Value = MetalPlane;
    type View = plane::View;
    type StringBuf = itoa::Buffer;

    fn get(
        t: &MetalDrmDeviceData,
        _key: u64,
    ) -> impl GetInner<HashMap<Self::Key, Rc<Self::Value>, Self::BuildHasher>> {
        &t.dev.planes
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(key.0)
    }

    fn parse_name(name: &str) -> Option<Self::Key> {
        u32::from_str(name).ok().map(DrmPlane)
    }
}

impl connector::Dir for MetalConnector {
    type ViewActiveFramebuffer = framebuffer::View;
    type ViewPossibleCrtcs = BinarySearchMapDir<PossibleCrtcs>;

    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.0.str_fmt(buf, ctx);
    }

    fn read_kernel_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.kernel_id.get().str_fmt(buf, ctx);
    }

    fn read_connector_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.connector_id.raw().str_fmt(buf, ctx);
    }

    fn read_buffers_idle(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.buffers_idle.get().str_fmt(buf, ctx);
    }

    fn read_crtc_idle(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.crtc_idle.get().str_fmt(buf, ctx);
    }

    fn read_has_damage(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.has_damage.get().str_fmt(buf, ctx);
    }

    fn read_cursor_changed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_changed.get().str_fmt(buf, ctx);
    }

    fn read_cursor_damage(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_damage.get().str_fmt(buf, ctx);
    }

    fn read_cursor_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_enabled.get().str_fmt(buf, ctx);
    }

    fn read_cursor_x(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_x.get().str_fmt(buf, ctx);
    }

    fn read_cursor_y(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_y.get().str_fmt(buf, ctx);
    }

    fn read_next_vblank_nsec(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.next_vblank_nsec.get().str_fmt(buf, ctx);
    }

    fn read_frontend_state(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let state = match self.frontend_state.get() {
            FrontState::Removed => "removed",
            FrontState::Disconnected => "disconnected",
            FrontState::Connected { .. } => "connected",
            FrontState::Unavailable => "unavailable",
        };
        state.str_fmt(buf, ctx);
    }

    fn read_frontend_non_desktop(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let non_desktop = match self.frontend_state.get() {
            FrontState::Connected { non_desktop } => Some(non_desktop),
            _ => None,
        };
        non_desktop.str_fmt(buf, ctx);
    }

    fn read_lease(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.lease.get().map(|l| l.raw()).str_fmt(buf, ctx);
    }

    fn read_has_buffers(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.buffers.get().is_some().str_fmt(buf, ctx);
    }

    fn read_has_cursor_buffers(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_buffers.get().is_some().str_fmt(buf, ctx);
    }

    fn read_has_cursor_sync(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_sync.get().is_some().str_fmt(buf, ctx);
    }

    fn read_cursor_swap_buffer(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_swap_buffer.get().str_fmt(buf, ctx);
    }

    fn read_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.version.get().str_fmt(buf, ctx);
    }

    fn read_expected_sequence(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.expected_sequence.get().str_fmt(buf, ctx);
    }

    fn read_pre_commit_margin(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pre_commit_margin.get().str_fmt(buf, ctx);
    }

    fn read_pre_commit_margin_decay(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pre_commit_margin_decay.get().str_fmt(buf, ctx);
    }

    fn read_post_commit_margin(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.post_commit_margin.get().str_fmt(buf, ctx);
    }

    fn read_post_commit_margin_decay(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.post_commit_margin_decay.get().str_fmt(buf, ctx);
    }

    fn read_vblank_miss_sec(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.vblank_miss_sec.get().str_fmt(buf, ctx);
    }

    fn read_vblank_miss_this_sec(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.vblank_miss_this_sec.get().str_fmt(buf, ctx);
    }

    fn read_presentation_is_sync(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.presentation_is_sync.get().str_fmt(buf, ctx);
    }

    fn read_presentation_is_zero_copy(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.presentation_is_zero_copy.get().str_fmt(buf, ctx);
    }

    fn read_direct_scanout_active(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.direct_scanout_active.get().str_fmt(buf, ctx);
    }

    fn read_last_direct_scanout_error(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        match self.last_direct_scanout_error.get() {
            None => None::<&str>.str_fmt(buf, ctx),
            Some(e) => ErrorFmt(e).to_string().as_str().str_fmt(buf, ctx),
        }
    }

    fn has_active_framebuffer(&self, _key: u64) -> bool {
        self.active_framebuffer.borrow().is_some()
    }

    fn read_has_gamma_lut(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.gamma_lut.get().is_some().str_fmt(buf, ctx);
    }

    fn read_color_description(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.color_description.get().str_fmt(buf, ctx);
    }

    fn read_fb_color_description(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.fb_color_description.get().str_fmt(buf, ctx);
    }

    fn read_fb_render_intent(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.fb_render_intent.get().text().str_fmt(buf, ctx);
    }

    fn read_connection(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let connection = match self.display.borrow().connection {
            ConnectorStatus::Connected => "connected",
            ConnectorStatus::Disconnected => "disconnected",
            ConnectorStatus::Unknown => "unknown",
            ConnectorStatus::Other(n) => {
                n.str_fmt(buf, ctx);
                return;
            }
        };
        connection.str_fmt(buf, ctx);
    }

    fn read_mm_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().mm_width.str_fmt(buf, ctx);
    }

    fn read_mm_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().mm_height.str_fmt(buf, ctx);
    }

    fn read_refresh(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().refresh.str_fmt(buf, ctx);
    }

    fn read_non_desktop(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().non_desktop.str_fmt(buf, ctx);
    }

    fn read_non_desktop_effective(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .non_desktop_effective
            .str_fmt(buf, ctx);
    }

    fn read_vrr_capable(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().vrr_capable.str_fmt(buf, ctx);
    }

    fn read_supports_bt2020(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().supports_bt2020.str_fmt(buf, ctx);
    }

    fn read_supports_pq(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().supports_pq.str_fmt(buf, ctx);
    }

    fn read_mode_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().first_mode.width.str_fmt(buf, ctx);
    }

    fn read_mode_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().first_mode.height.str_fmt(buf, ctx);
    }

    fn read_mode_refresh_rate_millihz(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .first_mode
            .refresh_rate_millihz
            .str_fmt(buf, ctx);
    }

    fn read_drm_locked(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.locked.str_fmt(buf, ctx);
    }

    fn read_drm_fb(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.fb.id().str_fmt(buf, ctx);
    }

    fn read_drm_fb_idx(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.fb_idx.str_fmt(buf, ctx);
    }

    fn read_drm_cursor_fb(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .drm_state
            .cursor_fb
            .id()
            .str_fmt(buf, ctx);
    }

    fn read_drm_cursor_fb_idx(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .drm_state
            .cursor_fb_idx
            .str_fmt(buf, ctx);
    }

    fn read_drm_cursor_x(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.cursor_x.str_fmt(buf, ctx);
    }

    fn read_drm_cursor_y(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.cursor_y.str_fmt(buf, ctx);
    }

    fn read_drm_src_w(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.src_w.str_fmt(buf, ctx);
    }

    fn read_drm_src_h(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.src_h.str_fmt(buf, ctx);
    }

    fn read_drm_crtc_x(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.crtc_x.str_fmt(buf, ctx);
    }

    fn read_drm_crtc_y(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.crtc_y.str_fmt(buf, ctx);
    }

    fn read_drm_crtc_w(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.crtc_w.str_fmt(buf, ctx);
    }

    fn read_drm_crtc_h(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display.borrow().drm_state.crtc_h.str_fmt(buf, ctx);
    }

    fn read_drm_has_out_fd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .drm_state
            .out_fd
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_drm_link_status(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .drm_state
            .props
            .link_status
            .value
            .str_fmt(buf, ctx);
    }

    fn read_drm_crtc_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .drm_state
            .props
            .crtc_id
            .value
            .id()
            .str_fmt(buf, ctx);
    }

    fn read_drm_color_space(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .drm_state
            .props
            .color_space
            .map(|v| v.value)
            .str_fmt(buf, ctx);
    }

    fn read_drm_hdr_metadata_blob_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.display
            .borrow()
            .drm_state
            .props
            .hdr_metadata_blob_id
            .map(|v| v.value.id())
            .str_fmt(buf, ctx);
    }

    fn has_crtc(&self) -> bool {
        self.crtc.is_some()
    }

    fn readlink_crtc(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.crtc.get() {
            write_obj_link(buf, depth, t.master.dev(), CRTCS, t.id.0);
        }
    }

    fn has_primary_plane(&self) -> bool {
        self.primary_plane.is_some()
    }

    fn readlink_primary_plane(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.primary_plane.get() {
            write_obj_link(buf, depth, t.master.dev(), PLANES, t.id.0);
        }
    }

    fn has_cursor_plane(&self) -> bool {
        self.cursor_plane.is_some()
    }

    fn readlink_cursor_plane(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.cursor_plane.get() {
            write_obj_link(buf, depth, t.master.dev(), PLANES, t.id.0);
        }
    }
}

struct PossibleCrtcs;

impl BinarySearchMapDirView<MetalConnector> for PossibleCrtcs {
    type Key = DrmCrtc;
    type Value = MetalCrtc;
    type View = FuseLink<ObjLink>;
    type StringBuf = itoa::Buffer;
    type Map = BinarySearchMap<DrmCrtc, Rc<MetalCrtc>, 8>;

    fn get(t: &MetalConnector, _key: u64) -> impl GetInner<Self::Map> {
        Ref::map(t.display.borrow(), |display| &display.crtcs)
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(key.0)
    }

    fn parse_name(name: &str) -> Option<Self::Key> {
        u32::from_str(name).ok().map(DrmCrtc)
    }
}

impl crtc::Dir for MetalCrtc {
    type ViewPossiblePlanes = BinarySearchMapDir<PossiblePlanes>;

    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.0.str_fmt(buf, ctx);
    }

    fn read_idx(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.idx.str_fmt(buf, ctx);
    }

    fn read_master_fd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.master.raw().str_fmt(buf, ctx);
    }

    fn read_lease(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.lease.get().map(|l| l.raw()).str_fmt(buf, ctx);
    }

    fn read_out_fence_ptr(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.out_fence_ptr.0.str_fmt(buf, ctx);
    }

    fn read_gamma_lut_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.gamma_lut_size.str_fmt(buf, ctx);
    }

    fn read_sequence(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.sequence.get().str_fmt(buf, ctx);
    }

    fn read_have_queued_sequence(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.have_queued_sequence.get().str_fmt(buf, ctx);
    }

    fn read_needs_vblank_emulation(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.needs_vblank_emulation.get().str_fmt(buf, ctx);
    }

    fn read_assigned_connector(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .assigned_connector
            .id()
            .str_fmt(buf, ctx);
    }

    fn read_active(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.active.value.str_fmt(buf, ctx);
    }

    fn read_mode_blob_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .props
            .mode_blob_id
            .value
            .id()
            .str_fmt(buf, ctx);
    }

    fn read_vrr_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .props
            .vrr_enabled
            .value
            .str_fmt(buf, ctx);
    }

    fn read_has_mode_blob(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .mode_blob
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_mode_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .mode
            .as_ref()
            .map(|m| m.name.as_bstr())
            .str_fmt(buf, ctx);
    }

    fn read_mode_clock(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .mode
            .as_ref()
            .map(|m| m.clock)
            .str_fmt(buf, ctx);
    }

    fn read_mode_hdisplay(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .mode
            .as_ref()
            .map(|m| m.hdisplay)
            .str_fmt(buf, ctx);
    }

    fn read_mode_vdisplay(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .mode
            .as_ref()
            .map(|m| m.vdisplay)
            .str_fmt(buf, ctx);
    }

    fn read_mode_vrefresh(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .mode
            .as_ref()
            .map(|m| m.vrefresh)
            .str_fmt(buf, ctx);
    }

    fn read_mode_flags(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .mode
            .as_ref()
            .map(|m| m.flags)
            .str_fmt(buf, ctx);
    }

    fn read_mode_type(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .mode
            .as_ref()
            .map(|m| m.ty)
            .str_fmt(buf, ctx);
    }

    fn has_connector(&self) -> bool {
        self.connector.is_some()
    }

    fn readlink_connector(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.connector.get() {
            write_obj_link(buf, depth, t.master.dev(), CONNECTORS, t.id.0);
        }
    }

    fn has_pending_flip(&self) -> bool {
        self.pending_flip.is_some()
    }

    fn readlink_pending_flip(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.pending_flip.get() {
            write_obj_link(buf, depth, t.master.dev(), CONNECTORS, t.id.0);
        }
    }
}

struct PossiblePlanes;

impl BinarySearchMapDirView<MetalCrtc> for PossiblePlanes {
    type Key = DrmPlane;
    type Value = MetalPlane;
    type View = FuseLink<ObjLink>;
    type StringBuf = itoa::Buffer;
    type Map = BinarySearchMap<DrmPlane, Rc<MetalPlane>, 8>;

    fn get(t: &MetalCrtc, _key: u64) -> impl GetInner<Self::Map> {
        &t.possible_planes
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(key.0)
    }

    fn parse_name(name: &str) -> Option<Self::Key> {
        u32::from_str(name).ok().map(DrmPlane)
    }
}

impl encoder::Dir for MetalEncoder {
    type ViewCrtcs = HashMapDir<EncoderCrtcs>;

    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.0.str_fmt(buf, ctx);
    }
}

struct EncoderCrtcs;

impl HashMapDirView<MetalEncoder> for EncoderCrtcs {
    type BuildHasher = ahash::RandomState;
    type Key = DrmCrtc;
    type Value = MetalCrtc;
    type View = FuseLink<ObjLink>;
    type StringBuf = itoa::Buffer;

    fn get(
        t: &MetalEncoder,
        _key: u64,
    ) -> impl GetInner<HashMap<Self::Key, Rc<Self::Value>, Self::BuildHasher>> {
        &t.crtcs
    }

    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
        buf.format(key.0)
    }

    fn parse_name(name: &str) -> Option<Self::Key> {
        u32::from_str(name).ok().map(DrmCrtc)
    }
}

impl plane::Dir for MetalPlane {
    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.0.str_fmt(buf, ctx);
    }

    fn read_ty(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.ty.text().str_fmt(buf, ctx);
    }

    fn read_master_fd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.master.raw().str_fmt(buf, ctx);
    }

    fn read_possible_crtcs(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.possible_crtcs.str_fmt(buf, ctx);
    }

    fn read_lease(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.lease.get().map(|l| l.raw()).str_fmt(buf, ctx);
    }

    fn read_mode_w(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mode_w.get().str_fmt(buf, ctx);
    }

    fn read_mode_h(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mode_h.get().str_fmt(buf, ctx);
    }

    fn read_in_fence_fd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.in_fence_fd.0.str_fmt(buf, ctx);
    }

    fn read_formats(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let mut names: Vec<_> = self.formats.values().map(|f| f.format.name).collect();
        names.sort_unstable();
        names.str_fmt(buf, ctx);
    }

    fn has_assigned_crtc(&self) -> bool {
        self.drm_state.borrow().assigned_crtc.is_some()
    }

    fn readlink_assigned_crtc(&self, depth: u64, buf: &mut String) {
        let crtc = self.drm_state.borrow().assigned_crtc;
        write_obj_link(buf, depth, self.master.dev(), CRTCS, crtc.0);
    }

    fn read_has_buffers(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().buffers.is_some().str_fmt(buf, ctx);
    }

    fn read_fb_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state
            .borrow()
            .props
            .fb_id
            .value
            .id()
            .str_fmt(buf, ctx);
    }

    fn read_src_x(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.src_x.value.str_fmt(buf, ctx);
    }

    fn read_src_y(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.src_y.value.str_fmt(buf, ctx);
    }

    fn read_src_w(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.src_w.value.str_fmt(buf, ctx);
    }

    fn read_src_h(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.src_h.value.str_fmt(buf, ctx);
    }

    fn has_crtc(&self) -> bool {
        self.drm_state.borrow().props.crtc_id.value.is_some()
    }

    fn readlink_crtc(&self, depth: u64, buf: &mut String) {
        let crtc = self.drm_state.borrow().props.crtc_id.value;
        write_obj_link(buf, depth, self.master.dev(), CRTCS, crtc.0);
    }

    fn read_crtc_x(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.crtc_x.value.str_fmt(buf, ctx);
    }

    fn read_crtc_y(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.crtc_y.value.str_fmt(buf, ctx);
    }

    fn read_crtc_w(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.crtc_w.value.str_fmt(buf, ctx);
    }

    fn read_crtc_h(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_state.borrow().props.crtc_h.value.str_fmt(buf, ctx);
    }
}

impl MetalConnector {
    pub fn debugfs_external_link(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<FuseLink<ExternalConnectorLink>>()
            .without_key()
    }
}

struct ExternalConnectorLink;

impl FuseLinkView<MetalConnector> for ExternalConnectorLink {
    fn readlink(t: &MetalConnector, _key: u64, depth: u64, buf: &mut String) {
        write_obj_link2(buf, depth, t.master.dev(), CONNECTORS, t.id.0, true);
    }
}

macro_rules! framebuffer {
    ($slf:expr, $v:pat, $body:expr) => {
        if let Some($v) = &*$slf.active_framebuffer.borrow() {
            $body
        }
    };
}

impl framebuffer::Dir for MetalConnector {
    type ViewDirectScanoutData = direct_scanout_data::View;

    fn read_fb(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        framebuffer!(self, v, v.fb.id().0.str_fmt(buf, ctx));
    }

    fn read_fb_cd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        framebuffer!(self, v, v.fb_cd.str_fmt(buf, ctx));
    }

    fn read_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        #[derive(StrFmt)]
        struct Size {
            width: i32,
            height: i32,
        }
        framebuffer!(self, v, {
            let (width, height) = v.tex.size();
            Size { width, height }.str_fmt(buf, ctx);
        });
    }

    fn read_format(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        framebuffer!(self, v, v.tex.format().str_fmt(buf, ctx));
    }

    fn has_dmabuf(&self) -> bool {
        let mut has = false;
        framebuffer!(self, v, has = v.tex.dmabuf().is_some());
        has
    }

    fn read_dmabuf(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        framebuffer!(self, v, {
            if let Some(v) = v.tex.dmabuf() {
                v.str_fmt(buf, ctx);
            }
        });
    }

    fn read_locked(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        framebuffer!(self, v, v.locked.str_fmt(buf, ctx));
    }

    fn has_direct_scanout_data(&self, _key: u64) -> bool {
        self.active_framebuffer
            .borrow()
            .as_ref()
            .is_some_and(|v| v.direct_scanout_data.is_some())
    }
}

macro_rules! dsd {
    ($slf:expr, $v:pat, $body:expr) => {
        framebuffer!($slf, v, {
            if let Some($v) = &v.direct_scanout_data {
                $body
            }
        });
    };
}

impl direct_scanout_data::Dir for MetalConnector {
    fn read_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        #[derive(StrFmt)]
        struct Size {
            width: i32,
            height: i32,
        }
        dsd!(self, v, {
            let (width, height) = v.tex.size();
            Size { width, height }.str_fmt(buf, ctx);
        });
    }

    fn read_format(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(self, v, v.tex.format().str_fmt(buf, ctx));
    }

    fn has_dmabuf(&self) -> bool {
        let mut has = false;
        dsd!(self, v, has = v.tex.dmabuf().is_some());
        has
    }

    fn read_dmabuf(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(
            self,
            v,
            if let Some(v) = v.tex.dmabuf() {
                v.str_fmt(buf, ctx);
            }
        );
    }

    fn read_has_tex_resv(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(self, v, v.tex_resv.is_some().str_fmt(buf, ctx));
    }

    fn read_acquire_sync(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(self, v, v.acquire_sync.text().str_fmt(buf, ctx));
    }

    fn read_release_sync(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(self, v, v.release_sync.text().str_fmt(buf, ctx));
    }

    fn read_has_fb_resv(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(self, v, v.fb_resv.is_some().str_fmt(buf, ctx));
    }

    fn read_is_lazy(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(self, v, v.lazy.is_some().str_fmt(buf, ctx));
    }

    fn read_fb(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(self, v, v.fb.id().0.str_fmt(buf, ctx));
    }

    fn read_position(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dsd!(self, v, v.position.str_fmt(buf, ctx));
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_e28b4c53e005e8ff8c539caf40b168ab3d458f372bcf01ac20bdc33a8af18bd3.rs",
));
// FUSE GENERATED STOP
