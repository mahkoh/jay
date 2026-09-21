/*
dir wl_surface {
    @inherit dfs_node,
    @inherit dfs_object,
    role: reg,
    destroyed: reg,
    transactional: reg,
    extents: reg,
    buffer_offset: reg,
    need_extents_update: reg,
    need_extents_propagation: reg,
    buffer_scale: reg,
    buffer_transform: reg,
    src_rect: reg (opt),
    dst_size: reg (opt),
    input_region: reg (opt),
    opaque_region: reg (opt),
    is_opaque: reg,
    alpha: reg (opt),
    alpha_mode: reg,
    render_intent: reg,
    color_description: reg (opt),
    content_type: reg (opt),
    has_content_type_manager: reg,
    tearing: reg,
    xwayland_serial: reg (opt),
    fullscreen: reg (opt),
    ext_version: reg,
    commit_version: reg,
    latched_commit_version: reg,
    requested_serial: reg,
    flush_frame_requests: reg,
    clear_fifo_on_vblank: reg,
    unmap_scheduled: reg,
    num_frame_requests: reg,
    num_presentation_feedback: reg,
    num_latched_presentation_feedback: reg,
    num_subsurfaces: reg,
    num_cursors: reg,
    num_dnd_icons: reg,
    num_idle_inhibitors: reg,
    num_constraints: reg,
    num_text_input_connections: reg,
    num_dmabuf_feedback: reg,
    num_color_management_feedback: reg,
    has_shm_staging: reg,
    has_prime_buffer: reg,
    role_obj: link (opt),
    buffer: link (opt),
    viewport: link (opt),
    fractional_scale: link (opt),
    tearing_control: link (opt),
    syncobj_surface: link (opt),
    commit_timer: link (opt),
    fifo: link (opt),
    alpha_modifier: link (opt),
    color_management_surface: link (opt),
    color_representation_surface: link (opt),
    toplevel: custom (opt),
    pending: view (key = 0),
    subsurfaces: view (key = 0),
    input_popups: view (key = 0),
}

dir pending {
    ext_version: reg,
    buffer: reg (opt),
    has_prime_buffer: reg,
    offset: reg,
    opaque_region: reg (opt),
    input_region: reg (opt),
    num_frame_requests: reg,
    damage_full: reg,
    num_buffer_damage: reg,
    num_surface_damage: reg,
    num_presentation_feedback: reg,
    src_rect: reg (opt),
    dst_size: reg (opt),
    scale: reg (opt),
    transform: reg (opt),
    xwayland_serial: reg (opt),
    tearing: reg (opt),
    content_type: reg (opt),
    num_subsurfaces: reg,
    has_acquire_point: reg,
    has_release_point: reg,
    sync_file_acquire: reg (opt),
    has_sync_file_release: reg,
    alpha_multiplier: reg (opt),
    syncobj_sync: reg,
    fifo_barrier_set: reg,
    fifo_barrier_wait: reg,
    commit_time: reg (opt),
    color_description: reg (opt),
    color_description_intent: reg (opt),
    serial: reg (opt),
    alpha_mode: reg (opt),
}
*/
use crate::client::Client;
use crate::dfs::dfs_helpers::DfsObjectLink;
use crate::dfs::dfs_helpers::format_object_link;
use crate::fixed::Fixed;
use crate::ifs::wl_surface::WlSurface;
use crate::ifs::wl_surface::wl_surface_dfs_g_fuse::generated::pending;
use crate::ifs::wl_surface::wl_surface_dfs_g_fuse::generated::wl_surface;
use crate::object::Object;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::FuseLink;
use crate::utils::fuse::fuse_views::IterDirKeyed;
use crate::utils::fuse::fuse_views::IterDirKeyedView;
use crate::utils::static_text::StaticText;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use crate::wire::WlSurfaceId;
use jay_proc::StrFmt;
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;

impl WlSurface {
    pub(super) fn debugfs_obj(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<wl_surface::View>().without_key()
    }
}

impl wl_surface::Dir for WlSurface {
    type ViewPending = pending::View;
    type ViewSubsurfaces = IterDirKeyed<Subsurfaces>;
    type ViewInputPopups = IterDirKeyed<InputPopups>;

    fn read_role(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.role.get().name().str_fmt(buf, ctx);
    }

    fn read_destroyed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.destroyed.get().str_fmt(buf, ctx);
    }

    fn read_transactional(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.transactional.get().str_fmt(buf, ctx);
    }

    fn read_extents(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.extents.get().str_fmt(buf, ctx);
    }

    fn read_buffer_offset(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        Position {
            x: self.buf_x.get(),
            y: self.buf_y.get(),
        }
        .str_fmt(buf, ctx);
    }

    fn read_need_extents_update(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.need_extents_update.get().str_fmt(buf, ctx);
    }

    fn read_need_extents_propagation(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.need_extents_propagation.get().str_fmt(buf, ctx);
    }

    fn read_buffer_scale(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.buffer_scale.get().str_fmt(buf, ctx);
    }

    fn read_buffer_transform(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.buffer_transform.get().text().str_fmt(buf, ctx);
    }

    fn has_src_rect(&self) -> bool {
        self.src_rect.get().is_some()
    }

    fn read_src_rect(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(rect) = self.src_rect.get() {
            src_rect(rect).str_fmt(buf, ctx);
        }
    }

    fn has_dst_size(&self) -> bool {
        self.dst_size.get().is_some()
    }

    fn read_dst_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some((width, height)) = self.dst_size.get() {
            Size { width, height }.str_fmt(buf, ctx);
        }
    }

    fn has_input_region(&self) -> bool {
        self.input_region.is_some()
    }

    fn read_input_region(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(region) = self.input_region.get() {
            region.rects().str_fmt(buf, ctx);
        }
    }

    fn has_opaque_region(&self) -> bool {
        self.opaque_region.is_some()
    }

    fn read_opaque_region(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(region) = self.opaque_region.get() {
            region.rects().str_fmt(buf, ctx);
        }
    }

    fn read_is_opaque(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.is_opaque.get().str_fmt(buf, ctx);
    }

    fn has_alpha(&self) -> bool {
        self.alpha.get().is_some()
    }

    fn read_alpha(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(alpha) = self.alpha.get() {
            alpha.str_fmt(buf, ctx);
        }
    }

    fn read_alpha_mode(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.alpha_mode.get().text().str_fmt(buf, ctx);
    }

    fn read_render_intent(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.render_intent.get().text().str_fmt(buf, ctx);
    }

    fn has_color_description(&self) -> bool {
        self.color_description.is_some()
    }

    fn read_color_description(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(cd) = self.color_description.get() {
            cd.str_fmt(buf, ctx);
        }
    }

    fn has_content_type(&self) -> bool {
        self.content_type.get().is_some()
    }

    fn read_content_type(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(ct) = self.content_type.get() {
            ct.text().str_fmt(buf, ctx);
        }
    }

    fn read_has_content_type_manager(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.has_content_type_manager.get().str_fmt(buf, ctx);
    }

    fn read_tearing(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tearing.get().str_fmt(buf, ctx);
    }

    fn has_xwayland_serial(&self) -> bool {
        self.xwayland_serial.get().is_some()
    }

    fn read_xwayland_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(serial) = self.xwayland_serial.get() {
            serial.str_fmt(buf, ctx);
        }
    }

    fn has_fullscreen(&self) -> bool {
        self.fullscreen.is_some()
    }

    fn read_fullscreen(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(id) = self.fullscreen.id() {
            id.raw().str_fmt(buf, ctx);
        }
    }

    fn read_ext_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.ext_version.get().str_fmt(buf, ctx);
    }

    fn read_commit_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.commit_version.get().str_fmt(buf, ctx);
    }

    fn read_latched_commit_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.latched_commit_version.get().str_fmt(buf, ctx);
    }

    fn read_requested_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.requested_serial.get().raw().str_fmt(buf, ctx);
    }

    fn read_flush_frame_requests(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.flush_frame_requests.get().str_fmt(buf, ctx);
    }

    fn read_clear_fifo_on_vblank(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.clear_fifo_on_vblank.get().str_fmt(buf, ctx);
    }

    fn read_unmap_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.unmap_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_num_frame_requests(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.frame_requests.borrow().len().str_fmt(buf, ctx);
    }

    fn read_num_presentation_feedback(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.presentation_feedback.borrow().len().str_fmt(buf, ctx);
    }

    fn read_num_latched_presentation_feedback(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.latched_presentation_feedback
            .borrow()
            .len()
            .str_fmt(buf, ctx);
    }

    fn read_num_subsurfaces(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let children = self.children.borrow();
        let num = match children.deref() {
            None => 0,
            Some(c) => c.subsurfaces.len(),
        };
        num.str_fmt(buf, ctx);
    }

    fn read_num_cursors(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursors.len().str_fmt(buf, ctx);
    }

    fn read_num_dnd_icons(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dnd_icons.len().str_fmt(buf, ctx);
    }

    fn read_num_idle_inhibitors(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.idle_inhibitors.len().str_fmt(buf, ctx);
    }

    fn read_num_constraints(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.constraints.len().str_fmt(buf, ctx);
    }

    fn read_num_text_input_connections(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.text_input_connections.len().str_fmt(buf, ctx);
    }

    fn read_num_dmabuf_feedback(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dmabuf_feedback.len().str_fmt(buf, ctx);
    }

    fn read_num_color_management_feedback(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.color_management_feedback.len().str_fmt(buf, ctx);
    }

    fn read_has_shm_staging(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.shm_staging.is_some().str_fmt(buf, ctx);
    }

    fn read_has_prime_buffer(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.prime.buffer.is_some().str_fmt(buf, ctx);
    }

    fn has_role_obj(&self) -> bool {
        self.ext.get().object_id().is_some()
    }

    fn readlink_role_obj(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.ext.get().object_id() {
            format_object_link(buf, depth, self.client.id, t);
        }
    }

    fn has_buffer(&self) -> bool {
        self.buffer.is_some()
    }

    fn readlink_buffer(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.buffer.get() {
            format_object_link(buf, depth, self.client.id, t.buffer.buf.id);
        }
    }

    fn has_viewport(&self) -> bool {
        self.viewporter.is_some()
    }

    fn readlink_viewport(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.viewporter.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn has_fractional_scale(&self) -> bool {
        self.fractional_scale.is_some()
    }

    fn readlink_fractional_scale(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.fractional_scale.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn has_tearing_control(&self) -> bool {
        self.tearing_control.is_some()
    }

    fn readlink_tearing_control(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.tearing_control.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn has_syncobj_surface(&self) -> bool {
        self.syncobj_surface.is_some()
    }

    fn readlink_syncobj_surface(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.syncobj_surface.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn has_commit_timer(&self) -> bool {
        self.commit_timer.is_some()
    }

    fn readlink_commit_timer(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.commit_timer.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn has_fifo(&self) -> bool {
        self.fifo.is_some()
    }

    fn readlink_fifo(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.fifo.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn has_alpha_modifier(&self) -> bool {
        self.alpha_modifier.is_some()
    }

    fn readlink_alpha_modifier(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.alpha_modifier.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn has_color_management_surface(&self) -> bool {
        self.color_management_surface.is_some()
    }

    fn readlink_color_management_surface(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.color_management_surface.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn has_color_representation_surface(&self) -> bool {
        self.color_representation_surface.is_some()
    }

    fn readlink_color_representation_surface(&self, depth: u64, buf: &mut String) {
        if let Some(obj) = self.color_representation_surface.get() {
            format_object_link(buf, depth, self.client.id, obj.id());
        }
    }

    fn get_toplevel(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        Some(self.toplevel.get()?.node_debugfs_dyn())
    }
}

#[derive(StrFmt)]
struct Position {
    x: i32,
    y: i32,
}

#[derive(StrFmt)]
struct Size {
    width: i32,
    height: i32,
}

#[derive(StrFmt)]
struct SrcRect {
    x: Fixed,
    y: Fixed,
    width: Fixed,
    height: Fixed,
}

fn src_rect(rect: [Fixed; 4]) -> SrcRect {
    SrcRect {
        x: rect[0],
        y: rect[1],
        width: rect[2],
        height: rect[3],
    }
}

fn write_none(buf: &mut String, ctx: &StrCtx<'_>) {
    None::<&str>.str_fmt(buf, ctx);
}

impl pending::Dir for WlSurface {
    fn read_ext_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().ext_version.str_fmt(buf, ctx);
    }

    fn has_buffer(&self) -> bool {
        self.pending.borrow().buffer.is_some()
    }

    fn read_buffer(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let pending = self.pending.borrow();
        if let Some(buffer) = &pending.buffer {
            buffer.as_ref().map(|b| b.buf.id.raw()).str_fmt(buf, ctx);
        }
    }

    fn read_has_prime_buffer(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending
            .borrow()
            .prime_buffer
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_offset(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let (x, y) = self.pending.borrow().offset;
        Position { x, y }.str_fmt(buf, ctx);
    }

    fn has_opaque_region(&self) -> bool {
        self.pending.borrow().opaque_region.is_some()
    }

    fn read_opaque_region(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let pending = self.pending.borrow();
        if let Some(region) = &pending.opaque_region {
            region.as_ref().map(|r| r.rects()).str_fmt(buf, ctx);
        }
    }

    fn has_input_region(&self) -> bool {
        self.pending.borrow().input_region.is_some()
    }

    fn read_input_region(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let pending = self.pending.borrow();
        if let Some(region) = &pending.input_region {
            region.as_ref().map(|r| r.rects()).str_fmt(buf, ctx);
        }
    }

    fn read_num_frame_requests(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().frame_request.len().str_fmt(buf, ctx);
    }

    fn read_damage_full(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().damage_full.str_fmt(buf, ctx);
    }

    fn read_num_buffer_damage(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().buffer_damage.len().str_fmt(buf, ctx);
    }

    fn read_num_surface_damage(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().surface_damage.len().str_fmt(buf, ctx);
    }

    fn read_num_presentation_feedback(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending
            .borrow()
            .presentation_feedback
            .len()
            .str_fmt(buf, ctx);
    }

    fn has_src_rect(&self) -> bool {
        self.pending.borrow().src_rect.is_some()
    }

    fn read_src_rect(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let pending = self.pending.borrow();
        if let Some(rect) = &pending.src_rect {
            match rect {
                Some(rect) => src_rect(*rect).str_fmt(buf, ctx),
                None => write_none(buf, ctx),
            }
        }
    }

    fn has_dst_size(&self) -> bool {
        self.pending.borrow().dst_size.is_some()
    }

    fn read_dst_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let pending = self.pending.borrow();
        if let Some(size) = &pending.dst_size {
            match size {
                Some((width, height)) => Size {
                    width: *width,
                    height: *height,
                }
                .str_fmt(buf, ctx),
                None => write_none(buf, ctx),
            }
        }
    }

    fn has_scale(&self) -> bool {
        self.pending.borrow().scale.is_some()
    }

    fn read_scale(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(scale) = self.pending.borrow().scale {
            scale.str_fmt(buf, ctx);
        }
    }

    fn has_transform(&self) -> bool {
        self.pending.borrow().transform.is_some()
    }

    fn read_transform(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(transform) = self.pending.borrow().transform {
            transform.text().str_fmt(buf, ctx);
        }
    }

    fn has_xwayland_serial(&self) -> bool {
        self.pending.borrow().xwayland_serial.is_some()
    }

    fn read_xwayland_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(serial) = self.pending.borrow().xwayland_serial {
            serial.str_fmt(buf, ctx);
        }
    }

    fn has_tearing(&self) -> bool {
        self.pending.borrow().tearing.is_some()
    }

    fn read_tearing(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(tearing) = self.pending.borrow().tearing {
            tearing.str_fmt(buf, ctx);
        }
    }

    fn has_content_type(&self) -> bool {
        self.pending.borrow().content_type.is_some()
    }

    fn read_content_type(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(content_type) = self.pending.borrow().content_type {
            content_type.map(|c| c.text()).str_fmt(buf, ctx);
        }
    }

    fn read_num_subsurfaces(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().subsurfaces.len().str_fmt(buf, ctx);
    }

    fn read_has_acquire_point(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending
            .borrow()
            .acquire_point
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_has_release_point(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending
            .borrow()
            .release_point
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn has_sync_file_acquire(&self) -> bool {
        self.pending.borrow().sync_file_acquire.is_some()
    }

    fn read_sync_file_acquire(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let pending = self.pending.borrow();
        if let Some(sync_file) = &pending.sync_file_acquire {
            sync_file.is_some().str_fmt(buf, ctx);
        }
    }

    fn read_has_sync_file_release(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending
            .borrow()
            .sync_file_release
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn has_alpha_multiplier(&self) -> bool {
        self.pending.borrow().alpha_multiplier.is_some()
    }

    fn read_alpha_multiplier(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(alpha) = self.pending.borrow().alpha_multiplier {
            alpha.str_fmt(buf, ctx);
        }
    }

    fn read_syncobj_sync(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().syncobj_sync.str_fmt(buf, ctx);
    }

    fn read_fifo_barrier_set(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().fifo_barrier_set.str_fmt(buf, ctx);
    }

    fn read_fifo_barrier_wait(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pending.borrow().fifo_barrier_wait.str_fmt(buf, ctx);
    }

    fn has_commit_time(&self) -> bool {
        self.pending.borrow().commit_time.is_some()
    }

    fn read_commit_time(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(time) = self.pending.borrow().commit_time {
            time.str_fmt(buf, ctx);
        }
    }

    fn has_color_description(&self) -> bool {
        self.pending.borrow().color_description.is_some()
    }

    fn read_color_description(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let pending = self.pending.borrow();
        if let Some(cd) = &pending.color_description {
            match cd {
                Some((_, cd)) => cd.str_fmt(buf, ctx),
                None => write_none(buf, ctx),
            }
        }
    }

    fn has_color_description_intent(&self) -> bool {
        self.pending.borrow().color_description.is_some()
    }

    fn read_color_description_intent(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let pending = self.pending.borrow();
        if let Some(cd) = &pending.color_description {
            cd.as_ref()
                .map(|(intent, _)| intent.text())
                .str_fmt(buf, ctx);
        }
    }

    fn has_serial(&self) -> bool {
        self.pending.borrow().serial.is_some()
    }

    fn read_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(serial) = self.pending.borrow().serial {
            serial.raw().str_fmt(buf, ctx);
        }
    }

    fn has_alpha_mode(&self) -> bool {
        self.pending.borrow().alpha_mode.is_some()
    }

    fn read_alpha_mode(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(alpha_mode) = self.pending.borrow().alpha_mode {
            alpha_mode.text().str_fmt(buf, ctx);
        }
    }
}

struct Subsurfaces;

impl IterDirKeyedView<WlSurface> for Subsurfaces {
    type Value = Client;
    type View = FuseLink<DfsObjectLink>;

    fn iter(t: Rc<WlSurface>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        let children = t.children.borrow();
        let Some(children) = children.deref() else {
            return;
        };
        let mut buf = itoa::Buffer::new();
        for (id, child) in &children.subsurfaces {
            f(buf.format(id.raw()), &child.surface.client, id.raw() as _);
        }
    }

    fn get(t: Rc<WlSurface>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
        let id = u64::from_str(name).ok()?;
        let children = t.children.borrow();
        let child = children
            .deref()
            .as_ref()?
            .subsurfaces
            .get(&WlSurfaceId::from_raw(id))?;
        Some((child.surface.client.clone(), id))
    }
}

struct InputPopups;

impl IterDirKeyedView<WlSurface> for InputPopups {
    type Value = Client;
    type View = FuseLink<DfsObjectLink>;

    fn iter(t: Rc<WlSurface>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        let mut buf = itoa::Buffer::new();
        for (_, con) in &t.text_input_connections {
            for (_, popup) in con.input_method.popups() {
                f(
                    buf.format(popup.surface.id.raw()),
                    &popup.surface.client,
                    popup.surface.id.raw() as _,
                );
            }
        }
    }

    fn get(t: Rc<WlSurface>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
        let id = u64::from_str(name).ok()?;
        for (_, con) in &t.text_input_connections {
            for (_, popup) in con.input_method.popups() {
                if popup.surface.id.raw() == id {
                    return Some((popup.surface.client.clone(), id));
                }
            }
        }
        None
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_3d078f88ba584bcd1f20e8fb395ab65a37f2f2f050ce5dd2cc7c22f3f7efe731.rs",
));
// FUSE GENERATED STOP
