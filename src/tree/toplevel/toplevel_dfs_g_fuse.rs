/*
dir dfs_toplevel_node (abstract, global) {
    @inherit dfs_node,
    tl_accepts_keyboard_focus: reg,
    tl_surface: link (opt),
    tl_pinned: reg,
    tl_admits_children: reg,
    tl_render_bounds: reg (opt),
    tl_self_active: reg,
    tl_client: link (opt),
    tl_active_surfaces: reg,
    tl_visible: reg,
    tl_parent_is_float: reg,
    tl_float: custom (opt),
    tl_float_width: reg,
    tl_float_height: reg,
    tl_min_height: reg (opt),
    tl_min_width: reg (opt),
    tl_max_height: reg (opt),
    tl_max_width: reg (opt),
    tl_self_pinned: reg,
    tl_is_fullscreen: reg,
    tl_self_or_ancestor_is_fullscreen: reg,
    tl_fullscreen_data: view (opt, key = 0),
    tl_workspace_live: link (opt),
    tl_workspace_render: link (opt),
    tl_workspace_type: reg,
    tl_title: reg,
    tl_mapped_during_iteration: reg,
    tl_content_size: reg,
    tl_desired_extents: reg,
    tl_wants_attention: reg,
    tl_requested_attention: reg,
    tl_app_id: reg,
    tl_identifier: reg,
    tl_handles: view (key = 0),
    tl_manager_handles: view (key = 0),
    tl_render_highlight: reg,
    tl_jay_toplevels: view (key = 0),
    tl_jay_screencasts: view (key = 0),
    tl_ext_copy_sessions: view (key = 0),
    tl_just_mapped_scheduled: reg,
    # tl_seat_foci: view (key = 0),
    tl_content_type: reg (opt),
    # tl_session, TODO
    tl_is_root_container: reg,
    tl_is_overlay_root_container: reg,
}

dir fullscreen_data {
    placeholder: custom (opt),
    workspace: link (opt),
}
 */
use crate::dfs::dfs_helpers::ClientObjectDir;
use crate::dfs::dfs_helpers::dfs_split_view;
use crate::dfs::dfs_helpers::format_client_link;
use crate::dfs::dfs_helpers::format_object_link;
use crate::dfs::dfs_helpers::format_workspace_link;
use crate::tree::NodeBase;
use crate::tree::ToplevelNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::toplevel::toplevel_dfs_g_fuse::generated::fullscreen_data;
use crate::tree::toplevel::toplevel_dfs_g_fuse::views::ExtCopySessions;
use crate::tree::toplevel::toplevel_dfs_g_fuse::views::Handles;
use crate::tree::toplevel::toplevel_dfs_g_fuse::views::JayScreencasts;
use crate::tree::toplevel::toplevel_dfs_g_fuse::views::JayToplevels;
use crate::tree::toplevel::toplevel_dfs_g_fuse::views::ManagerHandles;
use crate::utils::cell_ext::CellExt;
use crate::utils::fuse::fuse_globals::dfs_toplevel_node;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::static_text::StaticText;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use std::rc::Rc;

impl<T> dfs_toplevel_node::Dir for T
where
    T: ToplevelNode,
{
    type ViewTlFullscreenData = fullscreen_data::View;
    type ViewTlHandles = ClientObjectDir<Handles>;
    type ViewTlManagerHandles = ClientObjectDir<ManagerHandles>;
    type ViewTlJayToplevels = ClientObjectDir<JayToplevels>;
    type ViewTlJayScreencasts = ClientObjectDir<JayScreencasts>;
    type ViewTlExtCopySessions = ClientObjectDir<ExtCopySessions>;

    fn read_tl_accepts_keyboard_focus(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_accepts_keyboard_focus().str_fmt(buf, ctx);
    }

    fn has_tl_surface(&self) -> bool {
        self.tl_surface().is_some()
    }

    fn readlink_tl_surface(&self, depth: u64, buf: &mut String) {
        if let Some(v) = self.tl_surface() {
            format_object_link(buf, depth, v.client.id, v.id);
        }
    }

    fn read_tl_pinned(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_pinned().str_fmt(buf, ctx);
    }

    fn read_tl_admits_children(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_admits_children().str_fmt(buf, ctx);
    }

    fn has_tl_render_bounds(&self) -> bool {
        self.tl_render_bounds().is_some()
    }

    fn read_tl_render_bounds(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_render_bounds().str_fmt(buf, ctx);
    }

    fn read_tl_self_active(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().self_active.get().str_fmt(buf, ctx);
    }

    fn has_tl_client(&self) -> bool {
        self.tl_data().client.is_some()
    }

    fn readlink_tl_client(&self, depth: u64, buf: &mut String) {
        if let Some(v) = &self.tl_data().client {
            format_client_link(buf, depth, v.id);
        }
    }

    fn read_tl_active_surfaces(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().active_surfaces.active().str_fmt(buf, ctx);
    }

    fn read_tl_visible(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dfs_split_view(buf, ctx, |v| self.tl_data().visible[v].get());
    }

    fn read_tl_parent_is_float(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().parent_is_float.get().str_fmt(buf, ctx);
    }

    fn get_tl_float(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        Some(self.tl_data().float.get()?.node_debugfs())
    }

    fn read_tl_float_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().float_width.get().str_fmt(buf, ctx);
    }

    fn read_tl_float_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().float_height.get().str_fmt(buf, ctx);
    }

    fn has_tl_min_height(&self) -> bool {
        self.tl_data().min_height.get().is_some()
    }

    fn read_tl_min_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(v) = self.tl_data().min_height.get() {
            v.str_fmt(buf, ctx);
        }
    }

    fn has_tl_min_width(&self) -> bool {
        self.tl_data().min_width.get().is_some()
    }

    fn read_tl_min_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(v) = self.tl_data().min_width.get() {
            v.str_fmt(buf, ctx);
        }
    }

    fn has_tl_max_height(&self) -> bool {
        self.tl_data().max_height.get().is_some()
    }

    fn read_tl_max_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(v) = self.tl_data().max_height.get() {
            v.str_fmt(buf, ctx);
        }
    }

    fn has_tl_max_width(&self) -> bool {
        self.tl_data().max_width.get().is_some()
    }

    fn read_tl_max_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(v) = self.tl_data().max_width.get() {
            v.str_fmt(buf, ctx);
        }
    }

    fn read_tl_self_pinned(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().pinned.get().str_fmt(buf, ctx);
    }

    fn read_tl_is_fullscreen(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dfs_split_view(buf, ctx, |v| self.tl_data().is_fullscreen[v].get());
    }

    fn read_tl_self_or_ancestor_is_fullscreen(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data()
            .self_or_ancestor_is_fullscreen
            .get()
            .str_fmt(buf, ctx);
    }

    fn has_tl_fullscreen_data(&self, _key: u64) -> bool {
        self.tl_data().fullscrceen_data.borrow().is_some()
    }

    fn has_tl_workspace_live(&self) -> bool {
        self.tl_data().workspace[LiveTL].is_some()
    }

    fn readlink_tl_workspace_live(&self, depth: u64, buf: &mut String) {
        if let Some(ws) = self.tl_data().workspace[LiveTL].get() {
            format_workspace_link(buf, depth, &ws);
        }
    }

    fn has_tl_workspace_render(&self) -> bool {
        self.tl_data().workspace[RenderTL].is_some()
    }

    fn readlink_tl_workspace_render(&self, depth: u64, buf: &mut String) {
        if let Some(ws) = self.tl_data().workspace[RenderTL].get() {
            format_workspace_link(buf, depth, &ws);
        }
    }

    fn read_tl_workspace_type(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dfs_split_view(buf, ctx, |v| {
            self.tl_data().workspace_type[v].get().map(|v| v.text())
        });
    }

    fn read_tl_title(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().title.borrow().str_fmt(buf, ctx);
    }

    fn read_tl_mapped_during_iteration(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data()
            .mapped_during_iteration
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_tl_content_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().content_size.get().str_fmt(buf, ctx);
    }

    fn read_tl_desired_extents(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().desired_extents.get().str_fmt(buf, ctx);
    }

    fn read_tl_wants_attention(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().wants_attention.get().str_fmt(buf, ctx);
    }

    fn read_tl_requested_attention(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().requested_attention.get().str_fmt(buf, ctx);
    }

    fn read_tl_app_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().app_id.borrow().str_fmt(buf, ctx);
    }

    fn read_tl_identifier(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data()
            .identifier
            .get()
            .to_string()
            .str_fmt(buf, ctx);
    }

    fn read_tl_render_highlight(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().render_highlight.get().str_fmt(buf, ctx);
    }

    fn read_tl_just_mapped_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tl_data().just_mapped_scheduled.get().str_fmt(buf, ctx);
    }

    fn has_tl_content_type(&self) -> bool {
        self.tl_data().content_type.is_some()
    }

    fn read_tl_content_type(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(v) = self.tl_data().content_type.get() {
            v.text().str_fmt(buf, ctx);
        }
    }

    fn read_tl_is_root_container(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dfs_split_view(buf, ctx, |v| self.tl_data().is_root_container[v].get());
    }

    fn read_tl_is_overlay_root_container(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dfs_split_view(buf, ctx, |v| {
            self.tl_data().is_overlay_root_container[v].get()
        });
    }
}

impl<T> fullscreen_data::Dir for T
where
    T: ToplevelNode,
{
    fn get_placeholder(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        if let Some(v) = &*self.tl_data().fullscrceen_data.borrow() {
            return Some(v.placeholder.clone().node_debugfs());
        }
        None
    }

    fn has_workspace(&self) -> bool {
        self.tl_data().fullscrceen_data.borrow().is_some()
    }

    fn readlink_workspace(&self, depth: u64, buf: &mut String) {
        if let Some(v) = &*self.tl_data().fullscrceen_data.borrow() {
            format_workspace_link(buf, depth, &v.workspace);
        }
    }
}

mod views {
    use crate::client::Client;
    use crate::client::ClientId;
    use crate::dfs::dfs_helpers::ClientObjectDirView;
    use crate::ifs::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
    use crate::ifs::ext_image_copy::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1;
    use crate::ifs::jay_screencast::JayScreencast;
    use crate::ifs::jay_toplevel::JayToplevel;
    use crate::ifs::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1;
    use crate::tree::ToplevelNode;
    use crate::utils::copyhashmap::CopyHashMap;
    use crate::wire::ExtForeignToplevelHandleV1Id;
    use crate::wire::ExtImageCopyCaptureSessionV1Id;
    use crate::wire::JayScreencastId;
    use crate::wire::JayToplevelId;
    use crate::wire::ZwlrForeignToplevelHandleV1Id;
    use std::rc::Rc;

    pub struct Handles;

    impl<T> ClientObjectDirView<T> for Handles
    where
        T: ToplevelNode,
    {
        type ObjectId = ExtForeignToplevelHandleV1Id;
        type Value = ExtForeignToplevelHandleV1;
        type RandomState = ahash::RandomState;

        fn get(
            t: &T,
        ) -> &CopyHashMap<(ClientId, Self::ObjectId), Rc<Self::Value>, Self::RandomState> {
            &t.tl_data().handles
        }

        fn client(v: &Self::Value) -> &Rc<Client> {
            &v.client
        }
    }

    pub struct ManagerHandles;

    impl<T> ClientObjectDirView<T> for ManagerHandles
    where
        T: ToplevelNode,
    {
        type ObjectId = ZwlrForeignToplevelHandleV1Id;
        type Value = ZwlrForeignToplevelHandleV1;
        type RandomState = ahash::RandomState;

        fn get(
            t: &T,
        ) -> &CopyHashMap<(ClientId, Self::ObjectId), Rc<Self::Value>, Self::RandomState> {
            &t.tl_data().manager_handles
        }

        fn client(v: &Self::Value) -> &Rc<Client> {
            &v.client
        }
    }

    pub struct JayToplevels;

    impl<T> ClientObjectDirView<T> for JayToplevels
    where
        T: ToplevelNode,
    {
        type ObjectId = JayToplevelId;
        type Value = JayToplevel;
        type RandomState = ahash::RandomState;

        fn get(
            t: &T,
        ) -> &CopyHashMap<(ClientId, Self::ObjectId), Rc<Self::Value>, Self::RandomState> {
            &t.tl_data().jay_toplevels
        }

        fn client(v: &Self::Value) -> &Rc<Client> {
            &v.client
        }
    }

    pub struct JayScreencasts;

    impl<T> ClientObjectDirView<T> for JayScreencasts
    where
        T: ToplevelNode,
    {
        type ObjectId = JayScreencastId;
        type Value = JayScreencast;
        type RandomState = ahash::RandomState;

        fn get(
            t: &T,
        ) -> &CopyHashMap<(ClientId, Self::ObjectId), Rc<Self::Value>, Self::RandomState> {
            &t.tl_data().jay_screencasts
        }

        fn client(v: &Self::Value) -> &Rc<Client> {
            &v.client
        }
    }

    pub struct ExtCopySessions;

    impl<T> ClientObjectDirView<T> for ExtCopySessions
    where
        T: ToplevelNode,
    {
        type ObjectId = ExtImageCopyCaptureSessionV1Id;
        type Value = ExtImageCopyCaptureSessionV1;
        type RandomState = ahash::RandomState;

        fn get(
            t: &T,
        ) -> &CopyHashMap<(ClientId, Self::ObjectId), Rc<Self::Value>, Self::RandomState> {
            &t.tl_data().ext_copy_sessions
        }

        fn client(v: &Self::Value) -> &Rc<Client> {
            &v.client
        }
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_0febdb8a3c3dab7ff8a80e24b709c6b5724446981d18381655c0679119792954.rs",
));
// FUSE GENERATED STOP
