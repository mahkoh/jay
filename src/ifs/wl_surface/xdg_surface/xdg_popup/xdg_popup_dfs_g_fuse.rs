/*
dir xdg_popup {
    @inherit dfs_node,
    @inherit dfs_object,
    relative_position: reg,
    has_parent: reg,
    set_visible_prepared: reg,
    has_jay_popup_ext: reg,
    num_interactive_moves: reg,
    reposition_token: reg,
    size_width: reg,
    size_height: reg,
    anchor_rect: reg,
    anchor: reg,
    gravity: reg,
    constraint_adjustment: reg,
    off_x: reg,
    off_y: reg,
    reactive: reg,
    parent_width: reg,
    parent_height: reg,
    parent_serial: reg,
    xdg_surface: link,
    surface: link,
}
 */
use crate::dfs::dfs_helpers::format_object_link;
use crate::ifs::wl_surface::xdg_surface::xdg_popup::XdgPopup;
use crate::ifs::wl_surface::xdg_surface::xdg_popup::xdg_popup_dfs_g_fuse::generated::xdg_popup;
use crate::object::Object;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl XdgPopup {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<xdg_popup::View>().without_key()
    }
}

impl xdg_popup::Dir for XdgPopup {
    fn read_relative_position(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.relative_position.get().str_fmt(buf, ctx);
    }

    fn read_has_parent(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.parent.get().is_some().str_fmt(buf, ctx);
    }

    fn read_set_visible_prepared(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.set_visible_prepared.get().str_fmt(buf, ctx);
    }

    fn read_has_jay_popup_ext(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.jay_popup_ext.get().is_some().str_fmt(buf, ctx);
    }

    fn read_num_interactive_moves(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.interactive_moves.len().str_fmt(buf, ctx);
    }

    fn read_reposition_token(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.reposition_token.get().str_fmt(buf, ctx);
    }

    fn read_size_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().size_width.str_fmt(buf, ctx);
    }

    fn read_size_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().size_height.str_fmt(buf, ctx);
    }

    fn read_anchor_rect(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().ar.str_fmt(buf, ctx);
    }

    fn read_anchor(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().anchor.0.str_fmt(buf, ctx);
    }

    fn read_gravity(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().gravity.0.str_fmt(buf, ctx);
    }

    fn read_constraint_adjustment(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().ca.0.str_fmt(buf, ctx);
    }

    fn read_off_x(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().off_x.str_fmt(buf, ctx);
    }

    fn read_off_y(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().off_y.str_fmt(buf, ctx);
    }

    fn read_reactive(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().reactive.str_fmt(buf, ctx);
    }

    fn read_parent_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().parent_width.str_fmt(buf, ctx);
    }

    fn read_parent_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.borrow().parent_height.str_fmt(buf, ctx);
    }

    fn read_parent_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos
            .borrow()
            .parent_serial
            .map(|s| s.raw())
            .str_fmt(buf, ctx);
    }

    fn readlink_xdg_surface(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.xdg.surface.client.id, self.xdg.id());
    }

    fn readlink_surface(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.xdg.surface.client.id, self.xdg.surface.id);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_5086682b06b8b915013936314b70efa0cc72f5e6cd28382630745ce94a02a89b.rs",
));
// FUSE GENERATED STOP
