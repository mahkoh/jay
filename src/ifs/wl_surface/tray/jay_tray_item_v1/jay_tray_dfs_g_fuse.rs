/*
dir tray_item {
    @inherit dfs_node,
    @inherit dfs_object,
    tray_item_id: reg,
    visible: reg,
    attached: reg,
    destroyed: reg,
    sent_serial: reg,
    applied_serial: reg,
    num_popups: reg,
    abs_pos: reg,
    rel_pos_live: reg,
    rel_pos_render: reg,
    output: link (opt),
    surface: link,
    popups: view (key = 0),
}
 */
use crate::client::Client;
use crate::dfs::dfs_helpers::DfsCopyHashMapObjectLinkDir;
use crate::dfs::dfs_helpers::DfsCopyHashMapObjectLinkDirView;
use crate::dfs::dfs_helpers::DfsObjectCopyHashMap;
use crate::dfs::dfs_helpers::format_object_link;
use crate::dfs::dfs_helpers::format_output_link;
use crate::ifs::wl_surface::tray::jay_tray_item_v1::JayTrayItemV1;
use crate::ifs::wl_surface::tray::jay_tray_item_v1::jay_tray_dfs_g_fuse::generated::tray_item;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl JayTrayItemV1 {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<tray_item::View>().without_key()
    }
}

impl tray_item::Dir for JayTrayItemV1 {
    type ViewPopups = DfsCopyHashMapObjectLinkDir<Popups>;

    fn read_tray_item_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.tray_item_id.raw().str_fmt(buf, ctx);
    }

    fn read_visible(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.visible.get().str_fmt(buf, ctx);
    }

    fn read_attached(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.attached.get().str_fmt(buf, ctx);
    }

    fn read_destroyed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.destroyed.get().str_fmt(buf, ctx);
    }

    fn read_sent_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data
            .sent_serial
            .get()
            .map(|s| s.raw())
            .str_fmt(buf, ctx);
    }

    fn read_applied_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data
            .applied_serial
            .get()
            .map(|s| s.raw())
            .str_fmt(buf, ctx);
    }

    fn read_num_popups(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.popups.len().str_fmt(buf, ctx);
    }

    fn read_abs_pos(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.abs_pos.get().str_fmt(buf, ctx);
    }

    fn read_rel_pos_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.rel_pos[LiveTL].get().str_fmt(buf, ctx);
    }

    fn read_rel_pos_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.rel_pos[RenderTL].get().str_fmt(buf, ctx);
    }

    fn has_output(&self) -> bool {
        self.data.output.global.is_some()
    }

    fn readlink_output(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.data.output.global.get() {
            format_output_link(buf, depth, &t);
        }
    }

    fn readlink_surface(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.data.client.id, self.data.surface.id);
    }
}

struct Popups;

impl DfsCopyHashMapObjectLinkDirView<JayTrayItemV1> for Popups {
    fn client(t: &JayTrayItemV1) -> &Rc<Client> {
        &t.data.client
    }

    fn map(t: &JayTrayItemV1, _key: u64) -> &impl DfsObjectCopyHashMap {
        &t.popups
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_ee4946858eea08f26862b6c9452379cb91fd270b4c299ec979b4331fe0ccf092.rs",
));
// FUSE GENERATED STOP
