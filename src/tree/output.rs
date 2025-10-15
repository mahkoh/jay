use {
    crate::{
        backend::{
            BackendColorSpace, BackendConnectorState, BackendEotfs, BackendGammaLut, ButtonState,
            HardwareCursor, Mode, transaction::BackendConnectorTransactionError,
        },
        client::ClientId,
        cmm::{
            cmm_description::ColorDescription, cmm_eotf::Eotf, cmm_luminance::Luminance,
            cmm_primaries::NamedPrimaries,
        },
        control_center::{CCI_OUTPUTS, CCI_WORKSPACES},
        cursor::KnownCursor,
        cursor_user::{CursorUser, CursorUserId},
        damage::DamageMatrix,
        fixed::Fixed,
        gfx_api::{AcquireSync, BufferResv, GfxTexture, ReleaseSync},
        ifs::{
            color_management::wp_color_management_output_v1::WpColorManagementOutputV1,
            ext_image_copy::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
            jay_output::JayOutput,
            jay_screencast::JayScreencast,
            wl_buffer::WlBufferStorage,
            wl_output::{BlendSpace, PersistentOutputState, WlOutputGlobal},
            wl_seat::{
                BTN_LEFT, BTN_MIDDLE, NodeSeatState, SeatId, WlSeatGlobal,
                tablet::{TabletTool, TabletToolChanges, TabletToolId},
                wl_pointer::PendingScroll,
            },
            wl_surface::{
                SurfaceSendPreferredColorDescription, SurfaceSendPreferredScaleVisitor,
                SurfaceSendPreferredTransformVisitor,
                ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
                tray::DynTrayItem,
                zwlr_layer_surface_v1::{ExclusiveSize, ZwlrLayerSurfaceV1},
            },
            workspace_manager::{
                ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1,
                ext_workspace_manager_v1::WorkspaceManagerId,
            },
            wp_content_type_v1::ContentType,
            wp_presentation_feedback::KIND_VSYNC,
            zwlr_gamma_control_v1::ZwlrGammaControlV1,
            zwlr_layer_shell_v1::{BACKGROUND, BOTTOM, OVERLAY, TOP},
            zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        },
        output_schedule::OutputSchedule,
        rect::Rect,
        renderer::Renderer,
        scale::Scale,
        state::State,
        text::TextTexture,
        theme::BarPosition,
        tree::{
            Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId, NodeLayerLink,
            NodeLocation, NodesStack, PinnedNode, TddType, TileDragDestination, Transform,
            TreeSerial, WorkspaceDisplayOrder, WorkspaceDragDestination, WorkspaceInOutput,
            WorkspaceNode, WorkspaceType,
            transaction::{TreeTransaction, TreeTransactionOp, TreeTransactionTimeline},
            walker::NodeVisitor,
        },
        utils::{
            asyncevent::AsyncEvent,
            bitflags::BitflagsExt,
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            event_listener::EventSource,
            hash_map_ext::HashMapExt,
            linkedlist::{LinkedList, NodeRef},
            obj_and_id::{ObjAndId, ObjWithId},
            on_drop_event::OnDropEvent,
            ordered_float::F64,
            rc_eq::rc_eq,
            scroller::Scroller,
        },
        wire::{
            ExtImageCopyCaptureSessionV1Id, JayOutputId, JayScreencastId,
            WpColorManagementOutputV1Id, ZwlrScreencopyFrameV1Id,
        },
    },
    OutputTreeOpKind::*,
    ahash::AHashMap,
    jay_config::video::{TearingMode as ConfigTearingMode, VrrMode as ConfigVrrMode},
    numeric_sort::cmp,
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        ops::{BitOrAssign, Deref},
        rc::Rc,
    },
};

tree_id!(OutputNodeId);
pub struct OutputNode {
    pub id: OutputNodeId,
    pub global: Rc<WlOutputGlobal>,
    pub jay_outputs: CopyHashMap<(ClientId, JayOutputId), Rc<JayOutput>>,
    pub workspaces: LinkedList<WorkspaceInOutput>,
    pub seat_state: NodeSeatState,
    pub layers: [LinkedList<Rc<ZwlrLayerSurfaceV1>>; 4],
    pub exclusive_zones: Cell<ExclusiveSize>,
    pub render_data: RefCell<OutputRenderData>,
    pub state: Rc<State>,
    pub is_dummy: bool,
    pub status: CloneCell<Rc<String>>,
    pub scroll: Scroller,
    pub pointer_positions: CopyHashMap<PointerType, (i32, i32)>,
    pub pointer_down: CopyHashMap<SeatId, (i32, i32)>,
    pub hardware_cursor: CloneCell<Option<Rc<dyn HardwareCursor>>>,
    pub hardware_cursor_needs_render: Cell<bool>,
    pub update_render_data_scheduled: Cell<bool>,
    pub screencasts: CopyHashMap<(ClientId, JayScreencastId), Rc<JayScreencast>>,
    pub screencopies: CopyHashMap<(ClientId, ZwlrScreencopyFrameV1Id), Rc<ZwlrScreencopyFrameV1>>,
    pub title_visible: Cell<bool>,
    pub schedule: Rc<OutputSchedule>,
    pub latch_event: EventSource<dyn LatchListener>,
    pub vblank_event: EventSource<dyn VblankListener>,
    pub presentation_event: EventSource<dyn PresentationListener>,
    pub render_margin_ns: Cell<u64>,
    pub flip_margin_ns: Cell<Option<u64>>,
    pub ext_copy_sessions:
        CopyHashMap<(ClientId, ExtImageCopyCaptureSessionV1Id), Rc<ExtImageCopyCaptureSessionV1>>,
    pub before_latch_event: EventSource<dyn BeforeLatchListener>,
    pub tray_start_rel: Cell<i32>,
    pub tray_items: LinkedList<Rc<dyn DynTrayItem>>,
    pub ext_workspace_groups: CopyHashMap<WorkspaceManagerId, Rc<ExtWorkspaceGroupHandleV1>>,
    pub pinned: LinkedList<Rc<dyn PinnedNode>>,
    pub tearing: Cell<bool>,
    pub active_zwlr_gamma_control: CloneCell<Option<Rc<ZwlrGammaControlV1>>>,
    pub cursor_users: CopyHashMap<CursorUserId, Rc<CursorUser>>,
    pub current: OutputNodeState,
    pub mapped: OutputNodeState,
    pub color_description_listeners:
        CopyHashMap<(ClientId, WpColorManagementOutputV1Id), Rc<WpColorManagementOutputV1>>,
    pub timeline: TreeTransactionTimeline,
}

impl ObjWithId for Rc<OutputNode> {
    type Id = OutputNodeId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

pub struct OutputNodeState {
    pub pos: Cell<Rect>,
    pub transform: Cell<Transform>,
    pub scale: Cell<Scale>,
    pub legacy_scale: Cell<u32>,
    pub workspace: ObjAndId<Option<Rc<WorkspaceNode>>>,
    pub overlay: ObjAndId<Option<Rc<WorkspaceNode>>>,
    pub lock_surface: CloneCell<Option<Rc<ExtSessionLockSurfaceV1>>>,
    pub non_exclusive_rect: Cell<Rect>,
    pub non_exclusive_rect_rel: Cell<Rect>,
    pub brightness: Cell<Option<f64>>,
    pub blend_space: Cell<BlendSpace>,
    pub btf: Cell<BackendEotfs>,
    pub bcs: Cell<BackendColorSpace>,
    pub color_description: CloneCell<Rc<ColorDescription>>,
    pub linear_color_description: CloneCell<Rc<ColorDescription>>,
    pub connector_damage_matrix: Cell<DamageMatrix>,
    pub connector_damage_intersect: Cell<Rect>,
    pub workspace_rect: Cell<Rect>,
    pub workspace_rect_rel: Cell<Rect>,
    pub bar_rect: Cell<Rect>,
    pub bar_rect_rel: Cell<Rect>,
    pub bar_rect_with_separator: Cell<Rect>,
    pub bar_rect_with_separator_rel: Cell<Rect>,
    pub bar_separator_rect: Cell<Rect>,
    pub bar_separator_rect_rel: Cell<Rect>,
}

impl OutputNodeState {
    pub fn new(state: &Rc<State>) -> Self {
        Self {
            pos: Default::default(),
            transform: Default::default(),
            scale: Default::default(),
            legacy_scale: Default::default(),
            workspace: Default::default(),
            overlay: Default::default(),
            lock_surface: Default::default(),
            non_exclusive_rect: Default::default(),
            non_exclusive_rect_rel: Default::default(),
            brightness: Default::default(),
            blend_space: Default::default(),
            btf: Default::default(),
            bcs: Default::default(),
            color_description: CloneCell::new(state.color_manager.srgb_gamma22().clone()),
            linear_color_description: CloneCell::new(state.color_manager.srgb_linear().clone()),
            connector_damage_matrix: Default::default(),
            connector_damage_intersect: Default::default(),
            workspace_rect: Default::default(),
            workspace_rect_rel: Default::default(),
            bar_rect: Default::default(),
            bar_rect_rel: Default::default(),
            bar_rect_with_separator: Default::default(),
            bar_rect_with_separator_rel: Default::default(),
            bar_separator_rect: Default::default(),
            bar_separator_rect_rel: Default::default(),
        }
    }
}

impl OutputNodeState {
    fn clear(&self) {
        self.workspace.set(None);
        self.overlay.set(None);
        self.lock_surface.take();
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BeforeLatchResult {
    None,
    Yield,
}

impl BitOrAssign for BeforeLatchResult {
    fn bitor_assign(&mut self, rhs: Self) {
        if rhs == BeforeLatchResult::Yield {
            *self = rhs;
        }
    }
}

pub trait BeforeLatchListener {
    fn before_latch(self: Rc<Self>, present: u64) -> BeforeLatchResult;
}

pub trait LatchListener {
    fn after_latch(self: Rc<Self>, on: &OutputNode, tearing: bool);
}

pub trait VblankListener {
    fn after_vblank(self: Rc<Self>);
}

pub trait PresentationListener {
    fn presented(
        self: Rc<Self>,
        output: &OutputNode,
        tv_sec: u64,
        tv_nsec: u32,
        refresh: u32,
        seq: u64,
        flags: u32,
        vrr: bool,
    );
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PointerType {
    Seat(SeatId),
    TabletTool(TabletToolId),
}

pub async fn output_render_data(state: Rc<State>) {
    loop {
        let output = state.pending_output_render_data.pop().await;
        if output.global.destroyed.get() {
            continue;
        }
        if output.update_render_data_scheduled.get() {
            output.update_render_data_scheduled.set(false);
            output.update_render_data_phase1().triggered().await;
            output.update_render_data_phase2();
        }
    }
}

impl OutputNode {
    pub fn new(
        tt: &TreeTransaction,
        global: &Rc<WlOutputGlobal>,
        schedule: &Rc<OutputSchedule>,
    ) -> Rc<Self> {
        let persistent_state = &global.persistent;
        let (x, y) = persistent_state.pos.get();
        let mode = global.mode.get();
        let scale = global.persistent.scale.get();
        let (width, height) = calculate_logical_size(
            (mode.width, mode.height),
            persistent_state.transform.get(),
            scale,
        );
        let connector_state = &*global.connector.state.borrow();
        let on = Rc::new(OutputNode {
            id: global.state.node_ids.next(),
            workspaces: Default::default(),
            seat_state: Default::default(),
            global: global.clone(),
            layers: Default::default(),
            exclusive_zones: Default::default(),
            render_data: Default::default(),
            state: global.state.clone(),
            is_dummy: false,
            status: global.state.status.clone(),
            scroll: Default::default(),
            pointer_positions: Default::default(),
            pointer_down: Default::default(),
            hardware_cursor: Default::default(),
            jay_outputs: Default::default(),
            screencasts: Default::default(),
            update_render_data_scheduled: Cell::new(false),
            hardware_cursor_needs_render: Cell::new(false),
            screencopies: Default::default(),
            title_visible: Default::default(),
            schedule: schedule.clone(),
            latch_event: Default::default(),
            vblank_event: Default::default(),
            presentation_event: Default::default(),
            render_margin_ns: Default::default(),
            flip_margin_ns: Default::default(),
            ext_copy_sessions: Default::default(),
            before_latch_event: Default::default(),
            tray_start_rel: Default::default(),
            tray_items: Default::default(),
            ext_workspace_groups: Default::default(),
            pinned: Default::default(),
            tearing: Default::default(),
            active_zwlr_gamma_control: Default::default(),
            cursor_users: Default::default(),
            current: OutputNodeState::new(&global.state),
            mapped: OutputNodeState::new(&global.state),
            color_description_listeners: Default::default(),
            timeline: global.state.tree_transactions.timeline(),
        });
        on.set_current_pos(tt, Rect::new_sized(x, y, width, height).unwrap());
        on.set_current_transform(tt, persistent_state.transform.get());
        on.set_current_scale(tt, scale);
        on.set_current_legacy_scale(tt, scale.round_up());
        on.set_current_brightness(tt, persistent_state.brightness.get());
        on.set_current_blend_space(tt, persistent_state.blend_space.get());
        on.set_current_btf(tt, connector_state.eotf);
        on.set_current_bcs(tt, connector_state.color_space);
        on.update_visible(tt);
        on.update_rects(tt);
        on.update_damage_matrix(tt);
        on.update_color_description_(tt);
        on
    }

    pub async fn before_latch(&self, present: u64) {
        let mut res = BeforeLatchResult::None;
        for listener in self.before_latch_event.iter() {
            res |= listener.before_latch(present);
        }
        if res == BeforeLatchResult::Yield {
            self.state.eng.yield_now().await;
        }
    }

    pub fn latched(&self, tearing: bool) {
        self.schedule.latched();
        for listener in self.latch_event.iter() {
            listener.after_latch(self, tearing);
        }
    }

    pub fn vblank(&self) {
        for listener in self.vblank_event.iter() {
            listener.after_vblank();
        }
        if self.global.connector.needs_vblank_emulation.get() {
            if self.vblank_event.has_listeners() {
                self.global.connector.damage();
            } else {
                let connector = self.global.connector.clone();
                self.vblank_event.on_attach(Box::new(move || {
                    connector.damage();
                }));
            }
        }
    }

    pub fn presented(
        &self,
        tv_sec: u64,
        tv_nsec: u32,
        refresh: u32,
        seq: u64,
        flags: u32,
        vrr: bool,
        locked: bool,
    ) {
        for listener in self.presentation_event.iter() {
            listener.presented(self, tv_sec, tv_nsec, refresh, seq, flags, vrr);
        }
        if locked && let Some(lock) = self.state.lock.lock.get() {
            lock.check_locked()
        }
        let tearing = flags.not_contains(KIND_VSYNC);
        if self.tearing.replace(tearing) != tearing {
            self.global
                .connector
                .head_manager
                .handle_tearing_active_change(tearing);
            self.state.trigger_cci(CCI_OUTPUTS);
        }
    }

    pub fn update_exclusive_zones(self: &Rc<Self>, tt: &TreeTransaction) {
        let mut exclusive = ExclusiveSize::default();
        for layer in &self.layers {
            for surface in layer.iter() {
                exclusive = exclusive.max(&surface.exclusive_size());
            }
        }
        if self.exclusive_zones.replace(exclusive) != exclusive {
            self.update_rects(tt);
            for layer in &self.layers {
                for surface in layer.iter() {
                    surface.exclusive_zones_changed(tt);
                }
            }
            for layer in [&self.current.workspace, &self.current.overlay] {
                if let Some(c) = layer.get() {
                    c.change_extents(tt, &self.current.workspace_rect.get(), self);
                }
            }
            if self.node_visible() {
                self.state.damage(self.current.pos.get());
            }
        }
    }

    pub fn add_screencast(&self, tt: &TreeTransaction, sc: &Rc<JayScreencast>) {
        self.screencasts.set((sc.client.id, sc.id), sc.clone());
        self.screencast_changed(tt);
    }

    pub fn remove_screencast(&self, tt: &TreeTransaction, sc: &JayScreencast) {
        self.screencasts.remove(&(sc.client.id, sc.id));
        self.screencast_changed(tt);
    }

    pub fn screencast_changed(&self, tt: &TreeTransaction) {
        for ws in self.current_workspaces() {
            ws.update_has_captures(tt);
        }
    }

    pub fn perform_screencopies(
        &self,
        tex: &Rc<dyn GfxTexture>,
        cd: &Rc<ColorDescription>,
        resv: Option<&Rc<dyn BufferResv>>,
        acquire_sync: &AcquireSync,
        release_sync: ReleaseSync,
        render_hardware_cursor: bool,
        x_off: i32,
        y_off: i32,
        size: Option<(i32, i32)>,
    ) {
        if let Some(workspace) = self.current.workspace.get() {
            if !workspace.may_capture.get() {
                return;
            }
        }
        self.perform_wlr_screencopies(
            tex,
            cd,
            resv,
            acquire_sync,
            release_sync,
            render_hardware_cursor,
            x_off,
            y_off,
            size,
        );
        for sc in self.screencasts.lock().values() {
            sc.copy_texture(
                self,
                tex,
                cd,
                resv,
                acquire_sync,
                release_sync,
                render_hardware_cursor,
                x_off,
                y_off,
                size,
            );
        }
        for sc in self.ext_copy_sessions.lock().values() {
            sc.copy_texture(
                self,
                tex,
                cd,
                resv,
                acquire_sync,
                release_sync,
                render_hardware_cursor,
                x_off,
                y_off,
                size,
            );
        }
    }

    pub fn perform_wlr_screencopies(
        &self,
        tex: &Rc<dyn GfxTexture>,
        cd: &Rc<ColorDescription>,
        resv: Option<&Rc<dyn BufferResv>>,
        acquire_sync: &AcquireSync,
        release_sync: ReleaseSync,
        render_hardware_cursors: bool,
        x_off: i32,
        y_off: i32,
        size: Option<(i32, i32)>,
    ) {
        if self.screencopies.is_empty() {
            return;
        }
        let now = self.state.now();
        for capture in self.screencopies.lock().drain_values() {
            let wl_buffer = match capture.buffer.take() {
                Some(b) => b,
                _ => {
                    log::warn!("Capture frame is pending but has no buffer attached");
                    capture.send_failed();
                    continue;
                }
            };
            if wl_buffer.destroyed() {
                capture.send_failed();
                continue;
            }
            let mut ready = true;
            if let Some(storage) = wl_buffer.storage.borrow_mut().deref() {
                match storage {
                    WlBufferStorage::Shm { mem, stride, .. } => {
                        let res = self.state.perform_shm_screencopy(
                            tex,
                            cd,
                            acquire_sync,
                            self.current.pos.get(),
                            x_off,
                            y_off,
                            size,
                            &capture,
                            mem,
                            *stride,
                            wl_buffer.format,
                            self.current.transform.get(),
                            self.current.scale.get(),
                        );
                        match res {
                            Ok(p) => {
                                ready = p.is_none();
                                capture.pending.set(p);
                            }
                            Err(e) => {
                                log::warn!("Could not perform shm screencopy: {}", ErrorFmt(e));
                                capture.send_failed();
                                continue;
                            }
                        }
                    }
                    WlBufferStorage::Dmabuf { fb, .. } => {
                        let fb = match fb {
                            Some(fb) => fb,
                            _ => {
                                log::warn!("Capture buffer has no framebuffer");
                                capture.send_failed();
                                continue;
                            }
                        };
                        let res = self.state.perform_screencopy(
                            tex,
                            resv,
                            acquire_sync,
                            release_sync,
                            cd,
                            &fb,
                            AcquireSync::Implicit,
                            ReleaseSync::Implicit,
                            self.current.transform.get(),
                            self.state.color_manager.srgb_gamma22(),
                            self.current.pos.get(),
                            render_hardware_cursors,
                            x_off - capture.rect.x1(),
                            y_off - capture.rect.y1(),
                            size,
                            self.current.transform.get(),
                            self.current.scale.get(),
                        );
                        if let Err(e) = res {
                            log::warn!("Could not perform screencopy: {}", ErrorFmt(e));
                            capture.send_failed();
                            continue;
                        }
                    }
                }
            }
            if capture.with_damage.get() {
                capture.send_damage();
            }
            if ready {
                capture.send_ready(now.0.tv_sec as _, now.0.tv_nsec as _);
            }
        }
        let tt = &self.state.tree_transaction();
        self.screencast_changed(tt);
    }

    pub fn clear(&self) {
        self.global.clear();
        self.current.clear();
        self.mapped.clear();
        self.cursor_users.clear();
        let workspaces: Vec<_> = self.current_workspaces().collect();
        for workspace in workspaces {
            workspace.clear();
        }
        self.jay_outputs.clear();
        self.screencasts.clear();
        self.screencopies.clear();
        self.ext_copy_sessions.clear();
        self.ext_workspace_groups.clear();
        self.latch_event.clear();
        self.vblank_event.clear();
        self.presentation_event.clear();
        self.render_data.borrow_mut().clear();
        self.color_description_listeners.clear();
    }

    pub fn on_spaces_changed(self: &Rc<Self>, tt: &TreeTransaction) {
        self.update_rects(tt);
        for layer in [&self.current.workspace, &self.current.overlay] {
            if let Some(c) = layer.get() {
                c.change_extents(tt, &self.current.workspace_rect.get(), self);
            }
        }
        for item in self.tray_items.iter() {
            item.deref().clone().send_current_configure(tt);
        }
    }

    pub fn on_colors_changed(self: &Rc<Self>, tt: &TreeTransaction) {
        self.schedule_update_render_data(tt);
    }

    pub fn set_preferred_scale(self: &Rc<Self>, tt: &TreeTransaction, scale: Scale) {
        self.global.persistent.scale.set(scale);
        let old_scale = self.set_current_scale(tt, scale);
        if scale == old_scale {
            return;
        }
        let legacy_scale = scale.round_up();
        if self.set_current_legacy_scale(tt, legacy_scale) != legacy_scale {
            self.global.send_mode();
        }
        self.state.remove_output_scale(tt, old_scale);
        self.state.add_output_scale(tt, scale);
        let rect = self.calculate_extents();
        self.change_extents_(tt, &rect);
        self.visit_children(&mut SurfaceSendPreferredScaleVisitor);
        self.schedule_update_render_data(tt);
        self.global
            .connector
            .head_manager
            .handle_scale_change(scale);
        self.state.trigger_cci(CCI_OUTPUTS);
        for head in self.global.connector.wlr_output_heads.lock().values() {
            head.handle_new_scale(scale);
        }
    }

    pub fn schedule_update_render_data(self: &Rc<Self>, tt: &TreeTransaction) {
        self.add_op(tt, ScheduleUpdateRenderData);
    }

    fn update_render_data_phase1(self: &Rc<Self>) -> Rc<AsyncEvent> {
        let on_completed = Rc::new(OnDropEvent::default());
        if !self.state.show_bar.get() {
            return on_completed.event();
        }
        let Some(ctx) = self.state.render_ctx.get() else {
            return on_completed.event();
        };
        let font = self.state.theme.bar_font();
        let theme = &self.state.theme;
        let bh = theme.sizes.bar_height();
        let scale = self.current.scale.get();
        let scale = if scale != 1 {
            Some(scale.to_f64())
        } else {
            None
        };
        let mut texture_height = bh;
        if let Some(scale) = scale {
            texture_height = (bh as f64 * scale).round() as _;
        }
        let active_id = self.current.workspace.id();
        for ws in self.mapped_workspaces() {
            let tex = &mut *ws.title_texture.borrow_mut();
            let tex = tex.get_or_insert_with(|| TextTexture::new(&self.state, &ctx));
            let tc = match active_id == Some(ws.id) {
                true => theme.colors.focused_title_text.get(),
                false => theme.colors.unfocused_title_text.get(),
            };
            tex.schedule_render_fitting(
                on_completed.clone(),
                Some(texture_height),
                &font,
                &ws.name,
                tc,
                false,
                scale,
            );
        }
        if let Some(ws) = self.current.overlay.get() {
            let tex = &mut *ws.title_texture.borrow_mut();
            let tex = tex.get_or_insert_with(|| TextTexture::new(&self.state, &ctx));
            tex.schedule_render_fitting(
                on_completed.clone(),
                Some(texture_height),
                &font,
                &ws.name,
                theme.colors.focused_title_text.get(),
                false,
                scale,
            );
        }
        let mut rd = self.render_data.borrow_mut();
        let tex = rd.status.get_or_insert_with(|| OutputStatus {
            tex_x: 0,
            tex: TextTexture::new(&self.state, &ctx),
        });
        let status = self.status.get();
        let tc = self.state.theme.colors.bar_text.get();
        tex.tex.schedule_render_fitting(
            on_completed.clone(),
            Some(texture_height),
            &font,
            &status,
            tc,
            true,
            scale,
        );
        on_completed.event()
    }

    fn update_render_data_phase2(&self) {
        let mut rd = self.render_data.borrow_mut();
        rd.titles.clear();
        rd.inactive_workspaces.clear();
        rd.attention_requested_workspaces.clear();
        rd.captured_inactive_workspaces.clear();
        rd.active_workspace = None;
        rd.overlay_workspace = None;
        if !self.state.show_bar.get() {
            self.state.damage(rd.full_area);
            return;
        }
        let mut pos = 0;
        let bar_rect_rel = self.current.bar_rect_rel.get();
        let non_exclusive_rect_rel = self.current.non_exclusive_rect_rel.get();
        let y1 = bar_rect_rel.y1() - non_exclusive_rect_rel.y1();
        let scale = self.global.persistent.scale.get();
        let scale = if scale != 1 {
            Some(scale.to_f64())
        } else {
            None
        };
        let active_id = self.current.workspace.id();
        rd.bar_separator = self
            .current
            .bar_separator_rect_rel
            .get()
            .move_(-non_exclusive_rect_rel.x1(), -non_exclusive_rect_rel.y1());
        let mut handle_workspace = |ws: &Rc<WorkspaceNode>, overlay: bool| {
            let bh = bar_rect_rel.height();
            let mut title_width = bh;
            let mut icon_x = None;
            let mut x1 = pos;
            if overlay {
                if pos > 0 {
                    pos += 1;
                }
                x1 = pos;
                icon_x = Some(pos);
                pos += bh;
            }
            let title = &*ws.title_texture.borrow();
            if let Some(title) = title {
                if let Err(e) = title.flip() {
                    log::error!("Could not render title: {}", ErrorFmt(e));
                }
                if let Some(texture) = title.texture() {
                    let mut x = pos + 1;
                    let (mut width, _) = texture.size();
                    if let Some(scale) = scale {
                        width = (width as f64 / scale).round() as _;
                    }
                    if width + 2 > title_width {
                        title_width = width + 2;
                    } else {
                        x = pos + (title_width - width) / 2;
                    }
                    rd.titles.push(OutputTitle {
                        x1,
                        x2: pos + title_width,
                        icon_x,
                        tex_x: x,
                        tex_y: y1,
                        tex: texture,
                        ws: ws.clone(),
                    });
                }
            }
            let rect = if overlay {
                Rect::new_sized_saturating(pos - bh, y1, title_width + bh, bh)
            } else {
                Rect::new_sized_saturating(pos, y1, title_width, bh)
            };
            if overlay {
                rd.overlay_workspace = Some(rect);
            } else if Some(ws.id) == active_id {
                rd.active_workspace = Some(OutputWorkspaceRenderData {
                    rect,
                    captured: ws.has_capture.get(),
                });
            } else {
                if ws.attention_requests.active() {
                    rd.attention_requested_workspaces.push(rect);
                }
                if ws.has_capture.get() {
                    rd.captured_inactive_workspaces.push(rect);
                } else {
                    rd.inactive_workspaces.push(rect);
                }
            }
            pos += title_width;
        };
        for ws in self.mapped_workspaces() {
            handle_workspace(&ws, false);
        }
        if let Some(ws) = self.current.overlay.get() {
            handle_workspace(&ws, true);
        }
        if let Some(status) = &mut rd.status {
            if let Err(e) = status.tex.flip() {
                log::error!("Could not render status: {}", ErrorFmt(e));
            }
            if let Some(texture) = status.tex.texture() {
                let (mut width, _) = texture.size();
                if let Some(scale) = scale {
                    width = (width as f64 / scale).round() as _;
                }
                let pos = self.tray_start_rel.get() - width - 1;
                status.tex_x = pos;
            }
        }
        let old_full_area = rd.full_area;
        rd.full_area = self.current.bar_rect_with_separator.get();
        if self.title_visible.get() {
            self.state.damage(rd.full_area.union(old_full_area));
        }
    }

    pub fn ensure_workspace(self: &Rc<Self>, tt: &TreeTransaction) -> Rc<WorkspaceNode> {
        self.workspace()
            .unwrap_or_else(|| self.generate_normal_workspace(tt))
    }

    pub fn ensure_normal_workspace(self: &Rc<Self>, tt: &TreeTransaction) -> Rc<WorkspaceNode> {
        if let Some(ws) = self.current.workspace.get() {
            return ws;
        }
        if self.is_dummy
            && let Some(ws) = self.current_workspaces().last()
        {
            return ws;
        }
        self.generate_normal_workspace(tt)
    }

    pub fn generate_normal_workspace(self: &Rc<Self>, tt: &TreeTransaction) -> Rc<WorkspaceNode> {
        let name = 'name: {
            for i in 1.. {
                let name = i.to_string();
                if !self.state.workspaces.contains(&name) {
                    break 'name name;
                }
            }
            unreachable!();
        };
        self.create_normal_workspace(tt, &name)
    }

    pub fn workspace(&self) -> Option<Rc<WorkspaceNode>> {
        if let Some(ws) = self.current.overlay.get() {
            return Some(ws);
        }
        self.current.workspace.get()
    }

    pub fn show_workspace(self: &Rc<Self>, tt: &TreeTransaction, ws: &Rc<WorkspaceNode>) -> bool {
        if self.is_dummy {
            return false;
        }
        match ws.ty {
            WorkspaceType::Normal => self.show_normal_workspace(tt, ws),
            WorkspaceType::Overlay => self.show_overlay_workspace(tt, ws),
        }
    }

    fn show_normal_workspace(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        ws: &Rc<WorkspaceNode>,
    ) -> bool {
        let mut seats = SmallVec::new();
        if self.current.workspace.id() == Some(ws.id) {
            return false;
        }
        let old = self.set_current_workspace(tt, Some(ws));
        if self.current.overlay.is_none() {
            for user in self.cursor_users.lock().values() {
                user.workspace_changed(self, Some(ws));
            }
        }
        if let Some(old) = old {
            seats = old.collect_kb_foci();
            for pinned in self.pinned.iter() {
                pinned.deref().clone().set_workspace(tt, ws, false);
            }
            if old.is_empty() {
                for jw in old.jay_workspaces.lock().values() {
                    jw.send_destroyed();
                    jw.workspace.set(None);
                }
                for wh in old.ext_workspaces.lock().values() {
                    wh.handle_destroyed();
                }
                old.clear();
                self.state.workspaces.remove(&*old.name);
                self.state.trigger_cci(CCI_WORKSPACES);
            } else {
                old.set_visible(tt, false);
                old.flush_jay_workspaces();
            }
        }
        self.update_visible(tt);
        self.update_presentation_type(tt);
        if let Some(fs) = ws.current.fullscreen.get() {
            fs.tl_change_extents(tt, &self.current.pos.get());
        }
        ws.change_extents(tt, &self.current.workspace_rect.get(), self);
        for seat in seats {
            ws.do_focus(tt, &seat, Direction::Unspecified);
        }
        self.damage_if_visible(tt);
        true
    }

    fn show_overlay_workspace(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        ws: &Rc<WorkspaceNode>,
    ) -> bool {
        if self.current.overlay.id() == Some(ws.id) {
            return false;
        }
        if self.current.overlay.is_some() {
            self.move_pinned_to_normal_workspace(tt);
        }
        let mut seats = SmallVec::new();
        ws.current.output.get().hide_overlay2(tt, false, &mut seats);
        let old = self.current.overlay.set(Some(ws.clone()));
        if let Some(old) = &old {
            old.collect_kb_foci2(&mut seats);
        }
        for user in self.cursor_users.lock().values() {
            user.workspace_changed(self, Some(ws));
        }
        if let Some(old) = old {
            self.clear_old_overlay(tt, old);
        }
        self.update_visible(tt);
        self.update_presentation_type(tt);
        ws.set_output(tt, self);
        if let Some(fs) = ws.current.fullscreen.get() {
            fs.tl_change_extents(tt, &self.current.pos.get());
        }
        ws.change_extents(tt, &self.current.workspace_rect.get(), self);
        for seat in seats {
            ws.clone().node_do_focus(tt, &seat, Direction::Unspecified);
        }
        self.schedule_update_render_data(tt);
        if self.node_visible() {
            self.state.damage(self.current.pos.get());
        }
        true
    }

    pub fn hide_overlay(self: &Rc<Self>, tt: &TreeTransaction) {
        if self.current.overlay.is_none() {
            return;
        }
        let mut seats = SmallVec::new();
        self.hide_overlay2(tt, true, &mut seats);
        if let Some(ws) = self.workspace() {
            for seat in seats {
                ws.clone().node_do_focus(tt, &seat, Direction::Unspecified);
            }
        }
        self.schedule_update_render_data(tt);
        self.state.tree_changed();
    }

    fn hide_overlay2(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        clear_old: bool,
        seats: &mut SmallVec<[Rc<WlSeatGlobal>; 3]>,
    ) {
        if self.current.overlay.is_none() {
            return;
        }
        self.hide_overlay3(tt, clear_old, seats);
        self.update_visible(tt);
        self.update_presentation_type(tt);
        if self.node_visible() {
            self.state.damage(self.current.pos.get());
        }
    }

    fn hide_overlay3(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        clear_old: bool,
        seats: &mut SmallVec<[Rc<WlSeatGlobal>; 3]>,
    ) {
        let Some(ws) = self.current.overlay.set(None) else {
            return;
        };
        self.move_pinned_to_normal_workspace(tt);
        if clear_old {
            ws.collect_kb_foci2(seats);
        }
        for user in self.cursor_users.lock().values() {
            user.workspace_changed(self, self.workspace().as_ref());
        }
        if clear_old {
            self.clear_old_overlay(tt, ws);
        }
        self.schedule_update_render_data(tt);
    }

    fn clear_old_overlay(&self, tt: &TreeTransaction, ws: Rc<WorkspaceNode>) {
        if ws.is_empty() {
            ws.clear();
            self.state.workspaces.remove(&*ws.name);
            self.state.trigger_cci(CCI_WORKSPACES);
        } else {
            ws.set_visible(tt, false);
            ws.set_output(tt, &self.state.dummy_output.get().unwrap());
        }
    }

    fn move_pinned_to_normal_workspace(self: &Rc<Self>, tt: &TreeTransaction) {
        if self.pinned.is_not_empty() {
            let ws = self.ensure_normal_workspace(tt);
            for pinned in self.pinned.iter() {
                pinned.deref().clone().set_workspace(tt, &ws, false);
            }
        }
    }

    pub fn find_workspace_insertion_point(&self, name: &str) -> Option<NodeRef<WorkspaceInOutput>> {
        if self.state.workspace_display_order.get() == WorkspaceDisplayOrder::Sorted {
            for existing_ws in self.workspaces.iter() {
                if existing_ws.is_current_link.get()
                    && cmp(name, &existing_ws.name) == std::cmp::Ordering::Less
                {
                    return Some(existing_ws);
                }
            }
        }
        None
    }

    pub fn create_normal_workspace(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        name: &str,
    ) -> Rc<WorkspaceNode> {
        let ws = WorkspaceNode::new(self, name, WorkspaceType::Normal);
        ws.opt.set(Some(ws.clone()));
        ws.update_has_captures(tt);
        let wio = WorkspaceInOutput::new(&ws);
        let link = if let Some(before) = self.find_workspace_insertion_point(name) {
            before.prepend(wio)
        } else {
            self.workspaces.add_last(wio)
        };
        ws.set_current_output_link(tt, Some(link));
        self.state.workspaces.set(name.to_string(), ws.clone());
        self.state.trigger_cci(CCI_WORKSPACES);
        if self.current.workspace.is_none() {
            self.show_workspace(tt, &ws);
        }
        let mut clients_to_kill = AHashMap::new();
        for watcher in self.state.workspace_watchers.lock().values() {
            if let Err(e) = watcher.send_workspace(&ws) {
                clients_to_kill.insert(watcher.client.id, (watcher.client.clone(), e));
            }
        }
        for (client, e) in clients_to_kill.values() {
            client.error(e);
        }
        self.state.workspace_managers.announce_workspace(self, &ws);
        self.schedule_update_render_data(tt);
        ws
    }

    pub fn update_rects(self: &Rc<Self>, tt: &TreeTransaction) {
        let rect = self.current.pos.get();
        let bh = self.state.theme.sizes.bar_height();
        let bsw = self.state.theme.sizes.bar_separator_width();
        let exclusive = self.exclusive_zones.get();
        let y1 = rect.y1() + exclusive.top;
        let x2 = rect.x2() - exclusive.right;
        let y2 = rect.y2() - exclusive.bottom;
        let x1 = rect.x1() + exclusive.left;
        let width = (x2 - x1).max(0);
        let height = (y2 - y1).max(0);
        let non_exclusive_rect = Rect::new_sized_saturating(x1, y1, width, height);
        let non_exclusive_rect_rel =
            Rect::new_sized_saturating(exclusive.left, exclusive.top, width, height);
        let mut bar_rect = Rect::default();
        let mut bar_rect_rel = Rect::default();
        let mut bar_rect_with_separator = Rect::default();
        let mut bar_rect_with_separator_rel = Rect::default();
        let mut bar_separator_rect = Rect::default();
        let mut bar_separator_rect_rel = Rect::default();
        let mut workspace_rect = non_exclusive_rect;
        let mut workspace_rect_rel = non_exclusive_rect_rel;
        if self.state.show_bar.get() {
            match self.state.theme.bar_position.get() {
                BarPosition::Bottom => {
                    workspace_rect = Rect::new_sized_saturating(x1, y1, width, height - bh - bsw);
                    bar_rect_with_separator =
                        Rect::new_sized_saturating(x1, y1 + height - bh - bsw, width, bh + bsw);
                    bar_separator_rect =
                        Rect::new_sized_saturating(x1, y1 + height - bh - bsw, width, bsw);
                    bar_rect = Rect::new_sized_saturating(x1, y1 + height - bh, width, bh);
                }
                BarPosition::Top => {
                    bar_rect = Rect::new_sized_saturating(x1, y1, width, bh);
                    bar_separator_rect = Rect::new_sized_saturating(x1, y1 + bh, width, bsw);
                    bar_rect_with_separator = Rect::new_sized_saturating(x1, y1, width, bh + bsw);
                    workspace_rect =
                        Rect::new_sized_saturating(x1, y1 + bh + bsw, width, height - bh - bsw);
                }
            }
            let to_rel = |r: Rect| r.move_(-rect.x1(), -rect.y1());
            bar_rect_rel = to_rel(bar_rect);
            bar_rect_with_separator_rel = to_rel(bar_rect_with_separator);
            bar_separator_rect_rel = to_rel(bar_separator_rect);
            workspace_rect_rel = to_rel(workspace_rect);
        }
        self.current.non_exclusive_rect.set(non_exclusive_rect);
        self.current
            .non_exclusive_rect_rel
            .set(non_exclusive_rect_rel);
        self.current.bar_rect.set(bar_rect);
        self.current.bar_rect_rel.set(bar_rect_rel);
        self.current
            .bar_rect_with_separator
            .set(bar_rect_with_separator);
        self.current
            .bar_rect_with_separator_rel
            .set(bar_rect_with_separator_rel);
        self.current.bar_separator_rect.set(bar_separator_rect);
        self.current
            .bar_separator_rect_rel
            .set(bar_separator_rect_rel);
        self.current.workspace_rect.set(workspace_rect);
        self.current.workspace_rect_rel.set(workspace_rect_rel);
        self.set_current_non_exclusive_rect(tt, non_exclusive_rect, non_exclusive_rect_rel);
        self.update_tray_positions(tt);
        self.schedule_update_render_data(tt);
    }

    pub fn set_position(self: &Rc<Self>, tt: &TreeTransaction, x: i32, y: i32) {
        let pos = self.current.pos.get();
        if (pos.x1(), pos.y1()) == (x, y) {
            return;
        }
        let rect = pos.at_point(x, y);
        self.change_extents_(tt, &rect);
        for head in self.global.connector.wlr_output_heads.lock().values() {
            head.handle_position_change(x, y);
        }
    }

    pub fn update_mode(self: &Rc<Self>, tt: &TreeTransaction, mode: Mode) {
        self.update_mode_and_transform(tt, mode, self.current.transform.get());
    }

    pub fn update_transform(self: &Rc<Self>, tt: &TreeTransaction, transform: Transform) {
        self.update_mode_and_transform(tt, self.global.mode.get(), transform);
    }

    pub fn update_mode_and_transform(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        mode: Mode,
        transform: Transform,
    ) {
        let old_mode = self.global.mode.get();
        let old_transform = self.current.transform.get();
        if (old_mode, old_transform) == (mode, transform) {
            return;
        }
        let (old_width, old_height) = self.pixel_size();
        self.global.mode.set(mode);
        self.global.refresh_nsec.set(mode.refresh_nsec());
        self.global.persistent.transform.set(transform);
        self.set_current_transform(tt, transform);
        let (new_width, new_height) = self.pixel_size();
        self.change_extents_(tt, &self.calculate_extents());

        if (old_width, old_height) != (new_width, new_height) {
            for sc in self.screencasts.lock().values() {
                sc.schedule_realloc_or_reconfigure();
            }
            for sc in self.ext_copy_sessions.lock().values() {
                sc.buffer_size_changed();
            }
        }

        if transform != old_transform {
            self.state.refresh_hardware_cursors();
            self.node_visit_children(&mut SurfaceSendPreferredTransformVisitor);
            self.global
                .connector
                .head_manager
                .handle_transform_change(transform);
            self.state.trigger_cci(CCI_OUTPUTS);
            for head in self.global.connector.wlr_output_heads.lock().values() {
                head.hande_transform_change(transform);
            }
        }
    }

    fn calculate_extents(&self) -> Rect {
        Self::calculate_extents_(
            self.global.mode.get(),
            self.current.transform.get(),
            self.current.scale.get(),
            self.current.pos.get().position(),
        )
    }

    pub fn calculate_extents_(
        mode: Mode,
        transform: Transform,
        scale: Scale,
        pos: (i32, i32),
    ) -> Rect {
        let (width, height) = calculate_logical_size((mode.width, mode.height), transform, scale);
        Rect::new_sized_saturating(pos.0, pos.1, width, height)
    }

    fn change_extents_(self: &Rc<Self>, tt: &TreeTransaction, rect: &Rect) {
        self.damage_if_visible(tt);
        self.global.persistent.pos.set((rect.x1(), rect.y1()));
        self.set_current_pos(tt, *rect);
        self.update_damage_matrix(tt);
        self.damage_if_visible(tt);
        self.state.output_extents_changed();
        self.update_rects(tt);
        if let Some(ls) = self.current.lock_surface.get() {
            ls.change_extents(tt, *rect);
        }
        for layer in [&self.current.workspace, &self.current.overlay] {
            if let Some(c) = layer.get() {
                if let Some(fs) = c.current.fullscreen.get() {
                    fs.tl_change_extents(tt, rect);
                }
                c.change_extents(tt, &self.current.workspace_rect.get(), self);
            }
        }
        for layer in &self.layers {
            for surface in layer.iter() {
                surface.output_resized(tt);
            }
        }
        self.global.send_mode();
        for seat in self.state.globals.seats.lock().values() {
            seat.cursor_group().output_pos_changed(self)
        }
        self.state.tree_changed();
        self.global
            .connector
            .head_manager
            .handle_position_size_change(self);
        self.state.trigger_cci(CCI_OUTPUTS);
    }

    pub fn update_state(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        old: BackendConnectorState,
        state: BackendConnectorState,
    ) {
        self.update_btf_and_bcs(tt, state.eotf, state.color_space);
        if old.vrr != state.vrr {
            self.schedule.set_vrr_enabled(state.vrr);
        }
        if old.mode != state.mode {
            self.update_mode(tt, state.mode);
        }
        self.global.format.set(state.format);
    }

    fn update_btf_and_bcs(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        btf: BackendEotfs,
        bcs: BackendColorSpace,
    ) {
        let old_btf = self.set_current_btf(tt, btf);
        let old_bcs = self.set_current_bcs(tt, bcs);
        if (old_btf, old_bcs) == (btf, bcs) {
            return;
        }
        self.update_color_description(tt);
    }

    fn update_color_description(self: &Rc<Self>, tt: &TreeTransaction) {
        if self.update_color_description_(tt) {
            self.damage_if_visible(tt);
            self.add_op(tt, DamageHardwareCursor);
            for fb in self.color_description_listeners.lock().values() {
                fb.send_image_description_changed();
            }
            self.visit_children(&mut SurfaceSendPreferredColorDescription);
        }
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        self.node_visit_children(visitor);
        let root = &self.state.root;
        for layer in [
            &root.stacked,
            &root.stacked_above_layers,
            &root.stacked_in_overlay,
        ] {
            for stacked in layer.stacked.iter() {
                if stacked.node_output_id() != Some(self.id) {
                    continue;
                }
                stacked.deref().clone().node_visit(visitor);
            }
        }
    }

    pub fn set_brightness(self: &Rc<Self>, tt: &TreeTransaction, brightness: Option<f64>) {
        self.global.persistent.brightness.set(brightness);
        let old = self.set_current_brightness(tt, brightness);
        if old != brightness {
            self.update_color_description(tt);
            self.global
                .connector
                .head_manager
                .handle_brightness_change(brightness);
            self.state.trigger_cci(CCI_OUTPUTS);
        }
    }

    pub fn set_use_native_gamut(self: &Rc<Self>, tt: &TreeTransaction, use_native_gamut: bool) {
        let old = self
            .global
            .persistent
            .use_native_gamut
            .replace(use_native_gamut);
        if old != use_native_gamut {
            self.update_color_description(tt);
            self.global
                .connector
                .head_manager
                .handle_use_native_gamut_change(use_native_gamut);
            self.state.trigger_cci(CCI_OUTPUTS);
        }
    }

    pub fn set_blend_space(self: &Rc<Self>, tt: &TreeTransaction, blend_space: BlendSpace) {
        self.global.persistent.blend_space.set(blend_space);
        let old = self.set_current_blend_space(tt, blend_space);
        if old != blend_space {
            self.damage_if_visible(tt);
            self.global
                .connector
                .head_manager
                .handle_blend_space_change(blend_space);
            self.state.trigger_cci(CCI_OUTPUTS);
        }
    }

    fn find_stacked_at(
        &self,
        stack: &NodesStack,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        if stack.definitely_has_no_visible() {
            return FindTreeResult::Other;
        }
        let (x_abs, y_abs) = self.current.pos.get().translate_inv(x, y);
        for stacked in stack.iter_visible_rev() {
            let ext = stacked.node_mapped_position();
            if stacked.stacked_absolute_position_constrains_input() && !ext.contains(x_abs, y_abs) {
                // TODO: make constrain always true
                continue;
            }
            let (x, y) = ext.translate(x_abs, y_abs);
            let idx = tree.len();
            tree.push(FoundNode {
                node: stacked.deref().clone(),
                x,
                y,
            });
            match stacked.node_find_tree_at(x, y, tree, usecase) {
                FindTreeResult::AcceptsInput => {
                    return FindTreeResult::AcceptsInput;
                }
                FindTreeResult::Other => {
                    tree.truncate(idx);
                }
            }
        }
        FindTreeResult::Other
    }

    pub fn find_layer_surface_at(
        &self,
        x: i32,
        y: i32,
        layers: &[u32],
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        match usecase {
            FindTreeUsecase::None => {}
            FindTreeUsecase::SelectToplevel => return FindTreeResult::Other,
            FindTreeUsecase::SelectToplevelOrPopup => return FindTreeResult::Other,
            FindTreeUsecase::SelectNormalWorkspace => return FindTreeResult::Other,
        }
        let len = tree.len();
        for layer in layers.iter().copied() {
            for surface in self.layers[layer as usize].rev_iter() {
                let pos = surface.output_extents();
                if pos.contains(x, y) {
                    let (x, y) = pos.translate(x, y);
                    if surface.node_find_tree_at(x, y, tree, usecase)
                        == FindTreeResult::AcceptsInput
                    {
                        return FindTreeResult::AcceptsInput;
                    }
                    tree.truncate(len);
                }
            }
        }
        FindTreeResult::Other
    }

    pub fn set_status(self: &Rc<Self>, tt: &TreeTransaction, status: &Rc<String>) {
        self.status.set(status.clone());
        self.schedule_update_render_data(tt);
    }

    fn pointer_move(self: &Rc<Self>, id: PointerType, x: Fixed, y: Fixed) {
        self.pointer_positions
            .set(id, (x.round_down(), y.round_down()));
    }

    pub fn has_fullscreen(&self) -> bool {
        self.current
            .workspace
            .get()
            .map(|w| w.mapped.fullscreen.is_some())
            .unwrap_or(false)
            || self
                .current
                .overlay
                .get()
                .map(|w| w.current.fullscreen.is_some())
                .unwrap_or(false)
    }

    pub fn set_lock_surface(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        surface: Option<&Rc<ExtSessionLockSurfaceV1>>,
    ) -> Option<Rc<ExtSessionLockSurfaceV1>> {
        let prev = self.set_current_lock_surface(tt, surface);
        self.update_visible(tt);
        prev
    }

    pub fn fullscreen_changed(self: &Rc<Self>, tt: &TreeTransaction) {
        self.update_visible(tt);
        self.damage_if_visible(tt);
    }

    pub fn handle_workspace_display_order_update(self: &Rc<Self>, tt: &TreeTransaction) {
        if self.state.workspace_display_order.get() == WorkspaceDisplayOrder::Sorted {
            let mut workspaces: Vec<_> = self
                .workspaces
                .iter()
                .filter(|wio| wio.is_current_link.get())
                .collect();
            workspaces.sort_by(|a, b| cmp(&a.name, &b.name));
            for ws_ref in workspaces {
                ws_ref.detach();
                self.workspaces.add_last_existing(&ws_ref);
            }
        }
        self.schedule_update_render_data(tt);
    }

    pub fn update_visible(&self, tt: &TreeTransaction) {
        let mut visible = self.state.root_visible();
        if self.state.lock.locked.get() {
            if let Some(surface) = self.current.lock_surface.get() {
                surface.set_visible(visible);
            }
            visible = false;
        }
        macro_rules! set_layer_visible {
            ($layer:expr, $visible:expr) => {
                for ls in $layer.iter() {
                    ls.set_visible($visible);
                }
            };
        }
        let mut have_fullscreen = false;
        let mut have_overlay_fullscreen = false;
        if let Some(ws) = self.current.overlay.get() {
            have_fullscreen = ws.current.fullscreen.is_some();
            have_overlay_fullscreen = have_fullscreen;
        }
        if !have_fullscreen && let Some(ws) = self.current.workspace.get() {
            have_fullscreen = ws.current.fullscreen.is_some();
        }
        let lower_visible = visible && !have_fullscreen;
        self.title_visible.set(lower_visible);
        set_layer_visible!(self.layers[0], lower_visible);
        set_layer_visible!(self.layers[1], lower_visible);
        set_layer_visible!(self.layers[2], lower_visible);
        for item in self.tray_items.iter() {
            item.set_visible(lower_visible);
        }
        let ws_visible = visible && !have_overlay_fullscreen;
        if let Some(ws) = self.current.workspace.get() {
            ws.set_visible(tt, ws_visible);
        }
        set_layer_visible!(self.layers[3], ws_visible);
        if let Some(ws) = self.current.overlay.get() {
            ws.set_visible(tt, visible);
        }
    }

    fn bar_button(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, x: i32, y: i32, button: u32) -> bool {
        if !self.state.show_bar.get() {
            return false;
        }
        let bar_rect_rel = self.current.bar_rect_rel.get();
        if bar_rect_rel.not_contains(x, y) {
            return false;
        }
        let (x, _) = bar_rect_rel.translate(x, y);
        let ws = 'ws: {
            let rd = self.render_data.borrow_mut();
            for title in &rd.titles {
                if x >= title.x1 && x < title.x2 {
                    break 'ws title.ws.clone();
                }
            }
            return true;
        };
        let tt = &self.state.tree_transaction();
        if ws.ty == WorkspaceType::Overlay && button == BTN_MIDDLE {
            ws.current.output.get().hide_overlay(tt);
        } else {
            self.state.show_workspace2(tt, Some(seat), self, &ws);
        }
        true
    }

    fn button(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, id: PointerType, button: u32) {
        let (x, y) = match self.pointer_positions.get(&id) {
            Some(p) => p,
            _ => return,
        };
        if let PointerType::Seat(s) = id
            && button == BTN_LEFT
        {
            self.pointer_down.set(s, (x, y));
        }
        if self.bar_button(seat, x, y, button) {
            return;
        }
        if button != BTN_LEFT {
            return;
        }
        let bar_rect_with_separator_rel = self.current.bar_rect_with_separator_rel.get();
        if bar_rect_with_separator_rel.contains(x, y) {
            return;
        }
        let tt = &self.state.tree_transaction();
        let ws = self.ensure_normal_workspace(tt);
        seat.focus_node(ws);
    }

    pub fn update_presentation_type(&self, tt: &TreeTransaction) {
        self.update_vrr_state(tt);
        self.update_tearing(tt);
    }

    fn update_vrr_state(&self, tt: &TreeTransaction) {
        let enabled = match self.global.persistent.vrr_mode.get() {
            VrrMode::Never => false,
            VrrMode::Always => true,
            VrrMode::Fullscreen { surface } => 'get: {
                for layer in [&self.current.overlay, &self.current.workspace] {
                    let Some(ws) = layer.get() else {
                        continue;
                    };
                    let Some(tl) = ws.current.fullscreen.get() else {
                        continue;
                    };
                    if let Some(req) = surface {
                        let Some(surface) = tl.tl_scanout_surface() else {
                            break 'get false;
                        };
                        if let Some(req) = req.content_type {
                            let Some(content_type) = surface.content_type.get() else {
                                break 'get false;
                            };
                            match content_type {
                                ContentType::Photo if !req.photo => break 'get false,
                                ContentType::Video if !req.video => break 'get false,
                                ContentType::Game if !req.game => break 'get false,
                                _ => {}
                            }
                        }
                    }
                    break 'get true;
                }
                false
            }
        };
        let res = self
            .global
            .connector
            .modify_state(&self.state, tt, |s| s.vrr = enabled);
        if let Err(e) = res {
            log::error!("Could not set vrr mode: {}", e);
        }
    }

    fn update_tearing(&self, tt: &TreeTransaction) {
        let enabled = match self.global.persistent.tearing_mode.get() {
            TearingMode::Never => false,
            TearingMode::Always => true,
            TearingMode::Fullscreen { surface } => 'get: {
                for layer in [&self.current.overlay, &self.current.workspace] {
                    let Some(ws) = layer.get() else {
                        continue;
                    };
                    let Some(tl) = ws.current.fullscreen.get() else {
                        continue;
                    };
                    if let Some(req) = surface {
                        let Some(surface) = tl.tl_scanout_surface() else {
                            break 'get false;
                        };
                        if req.tearing_requested {
                            if !surface.tearing.get() {
                                break 'get false;
                            }
                        }
                    }
                    break 'get true;
                }
                false
            }
        };
        let res = self
            .global
            .connector
            .modify_state(&self.state, tt, |s| s.tearing = enabled);
        if let Err(e) = res {
            log::error!("Could not set tearing mode: {}", e);
        }
    }

    pub fn tile_drag_destination(
        self: &Rc<Self>,
        source: NodeId,
        x_abs: i32,
        y_abs: i32,
    ) -> Option<TileDragDestination> {
        if self.state.lock.locked.get() {
            return None;
        }
        for list in [
            &self.state.root.stacked_in_overlay,
            &self.state.root.stacked,
        ] {
            for stacked in list.iter_visible_rev() {
                let Some(float) = stacked.deref().clone().node_into_float() else {
                    continue;
                };
                let pos = float.node_mapped_position();
                if !pos.contains(x_abs, y_abs) {
                    continue;
                }
                return float.tile_drag_destination(source, x_abs, y_abs);
            }
        }
        if let Some(ws) = self.current.overlay.get() {
            if ws.current.fullscreen.is_some() {
                return None;
            }
            let rect = self.current.workspace_rect.get();
            if rect.contains(x_abs, y_abs) {
                let Some(c) = ws.current.container.get() else {
                    return Some(TileDragDestination {
                        highlight: rect,
                        ty: TddType::NewContainer { workspace: ws },
                    });
                };
                return c.tile_drag_destination(source, rect, x_abs, y_abs);
            }
        }
        let rect = self.current.non_exclusive_rect.get();
        if !rect.contains(x_abs, y_abs) {
            return None;
        }
        let Some(ws) = self.current.workspace.get() else {
            return Some(TileDragDestination {
                highlight: rect,
                ty: TddType::NewWorkspace {
                    output: self.clone(),
                },
            });
        };
        if ws.current.fullscreen.is_some() {
            return None;
        }
        let bar_rect_with_separator = self.current.bar_rect_with_separator.get();
        if bar_rect_with_separator.contains(x_abs, y_abs) {
            let rd = &*self.render_data.borrow();
            let bar_rect = self.current.bar_rect.get();
            let (x, _) = bar_rect.translate(x_abs, y_abs);
            let mut last_x2 = 0;
            for t in &rd.titles {
                if x < t.x2 {
                    return Some(TileDragDestination {
                        highlight: Rect::new_sized(
                            bar_rect.x1() + t.x1,
                            bar_rect.y1(),
                            t.x2 - t.x1,
                            bar_rect.height(),
                        )?,
                        ty: TddType::MoveToWorkspace {
                            workspace: t.ws.clone(),
                        },
                    });
                }
                last_x2 = t.x2;
            }
            return Some(TileDragDestination {
                highlight: Rect::new_sized(
                    bar_rect.x1() + last_x2,
                    bar_rect.y1(),
                    bar_rect.width() - last_x2,
                    bar_rect.height(),
                )?,
                ty: TddType::MoveToNewWorkspace {
                    output: self.clone(),
                },
            });
        }
        let rect = self.current.workspace_rect.get();
        if !rect.contains(x_abs, y_abs) {
            return None;
        }
        let Some(c) = ws.current.container.get() else {
            return Some(TileDragDestination {
                highlight: rect,
                ty: TddType::NewContainer { workspace: ws },
            });
        };
        if let Some(d) = ws.mapped.container.get()
            && rc_eq(&c, &d)
        {
            return c.tile_drag_destination(source, rect, x_abs, y_abs);
        }
        None
    }

    pub fn workspace_drag_destination(
        self: &Rc<Self>,
        source: &WorkspaceNode,
        x_abs: i32,
        y_abs: i32,
    ) -> Option<WorkspaceDragDestination> {
        if !self.state.show_bar.get() {
            return None;
        }
        let bar_rect_with_separator = self.current.bar_rect_with_separator.get();
        if bar_rect_with_separator.not_contains(x_abs, y_abs) {
            return None;
        }
        let bar_rect = self.current.bar_rect.get();
        if source.ty == WorkspaceType::Overlay {
            if self.current.overlay.id() == Some(source.id) {
                return None;
            }
            return Some(WorkspaceDragDestination {
                highlight: bar_rect,
                output: self.clone(),
                before: None,
            });
        }
        if self.state.workspace_display_order.get() == WorkspaceDisplayOrder::Sorted {
            if self.current_workspaces().any(|ws| ws.id == source.id) {
                return None;
            }
            return Some(WorkspaceDragDestination {
                highlight: bar_rect,
                output: self.clone(),
                before: None,
            });
        }
        let rd = &*self.render_data.borrow();
        let (x, _) = bar_rect.translate(x_abs, y_abs);
        let mut prev_is_source = false;
        let mut prev_center = 0;
        for t in &rd.titles {
            if t.ws.ty != WorkspaceType::Normal {
                break;
            }
            if t.ws.id == source.id {
                prev_is_source = true;
                continue;
            }
            let center = (t.x1 + t.x2) / 2;
            if x < center {
                return if prev_is_source {
                    None
                } else {
                    Some(WorkspaceDragDestination {
                        highlight: Rect::new_sized(
                            bar_rect.x1() + prev_center,
                            bar_rect.y1(),
                            center - prev_center,
                            bar_rect.height(),
                        )?,
                        output: self.clone(),
                        before: Some(t.ws.clone()),
                    })
                };
            }
            prev_center = center;
            prev_is_source = false;
        }
        if prev_is_source {
            return None;
        }
        return Some(WorkspaceDragDestination {
            highlight: Rect::new_sized(
                bar_rect.x1() + prev_center,
                bar_rect.y1(),
                bar_rect.width() - prev_center,
                bar_rect.height(),
            )?,
            output: self.clone(),
            before: None,
        });
    }

    pub fn update_tray_positions(self: &Rc<Self>, tt: &TreeTransaction) {
        let bar_rect = self.current.bar_rect.get();
        let mut right = bar_rect.width();
        let mut have_any = false;
        let icon_size = self.state.tray_icon_size();
        for item in self.tray_items.rev_iter() {
            if item.data().surface.buffer.is_none() {
                continue;
            }
            have_any = true;
            right -= bar_rect.height();
            let rel_pos = Rect::new_sized_saturating(right, 1, icon_size, icon_size);
            let abs_pos = rel_pos.move_(bar_rect.x1(), bar_rect.y1());
            item.set_position(abs_pos, rel_pos);
        }
        if have_any {
            right -= 2;
        }
        let prev_right = self.tray_start_rel.replace(right);
        if prev_right != right {
            {
                let min = prev_right.min(right);
                let rect = Rect::new_saturating(
                    bar_rect.x1() + min,
                    bar_rect.y1(),
                    bar_rect.x2(),
                    bar_rect.y2(),
                );
                self.state.damage(rect);
            }
            self.schedule_update_render_data(tt);
        }
        self.state.tree_changed();
    }

    pub fn set_vrr_mode(&self, tt: &TreeTransaction, mode: &VrrMode) {
        let old = self.global.persistent.vrr_mode.replace(*mode);
        if old != *mode {
            self.update_presentation_type(tt);
            self.global
                .connector
                .head_manager
                .handle_vrr_mode_change(mode);
            self.state.trigger_cci(CCI_OUTPUTS);
            for head in self.global.connector.wlr_output_heads.lock().values() {
                head.handle_vrr_mode_change(mode);
            }
        }
    }

    pub fn set_tearing_mode(&self, tt: &TreeTransaction, mode: &TearingMode) {
        let old = self.global.persistent.tearing_mode.replace(*mode);
        if old != *mode {
            self.update_presentation_type(tt);
            self.global
                .connector
                .head_manager
                .handle_tearing_mode_change(mode);
            self.state.trigger_cci(CCI_OUTPUTS);
        }
    }

    pub fn set_hardware_cursor(&self, hc: Option<Rc<dyn HardwareCursor>>) {
        let is_none = hc.is_none();
        let old = self.hardware_cursor.set(hc);
        let was_none = old.is_none();
        if was_none != is_none {
            if is_none {
                self.state.outputs_without_hc.fetch_add(1);
            } else {
                self.state.outputs_without_hc.fetch_sub(1);
            }
        }
    }

    pub fn take_keyboard_navigation_focus(
        &self,
        tt: &TreeTransaction,
        seat: &Rc<WlSeatGlobal>,
        direction: Direction,
    ) {
        for layer in [&self.current.overlay, &self.current.workspace] {
            let Some(ws) = layer.get() else {
                continue;
            };
            if let Some(fs) = ws.current.fullscreen.get() {
                if fs.node_visible() {
                    fs.node_do_focus(tt, seat, direction);
                    return;
                }
            } else if let Some(c) = ws.current.container.get() {
                if c.node_visible() {
                    c.node_do_focus(tt, seat, direction);
                    return;
                }
            } else if ws.ty == WorkspaceType::Normal {
                if ws.node_visible() {
                    seat.focus_node(ws);
                    return;
                }
            }
        }
    }

    pub fn set_gamma_lut(
        &self,
        tt: &TreeTransaction,
        gamma_lut: Option<Rc<BackendGammaLut>>,
    ) -> Result<(), BackendConnectorTransactionError> {
        self.global
            .connector
            .modify_state(&self.state, tt, |s| s.gamma_lut = gamma_lut)
            .inspect_err(|e| {
                log::error!("Could not set gamma_lut: {}", ErrorFmt(e));
            })
    }

    pub fn set_flip_margin(&self, margin_ns: u64) {
        self.flip_margin_ns.set(Some(margin_ns));
        self.state.trigger_cci(CCI_OUTPUTS);
    }

    pub fn current_workspaces(&self) -> impl Iterator<Item = Rc<WorkspaceNode>> {
        self.workspaces
            .iter()
            .filter(|ws| ws.is_current_link.get())
            .map(|ws| ws.ws.clone())
    }

    pub fn mapped_workspaces(&self) -> impl Iterator<Item = Rc<WorkspaceNode>> {
        self.workspaces
            .iter()
            .filter(|ws| ws.is_mapped_link.get())
            .map(|ws| ws.ws.clone())
    }

    pub fn pixel_size(&self) -> (i32, i32) {
        let mode = self.global.mode.get();
        self.current
            .transform
            .get()
            .maybe_swap((mode.width, mode.height))
    }

    pub fn update_damage_matrix(self: &Rc<Self>, tt: &TreeTransaction) {
        let pos = self.current.pos.get();
        let mode = self.global.mode.get();
        let matrix = DamageMatrix::new(
            self.current.transform.get().inverse(),
            1,
            pos.width(),
            pos.height(),
            None,
            mode.width,
            mode.height,
        );
        self.set_current_damage_matrix(
            tt,
            matrix,
            Rect::new_sized_saturating(0, 0, mode.width, mode.height),
        );
    }

    pub fn add_damage_area(&self, area: &Rect) {
        let pos = self.current.pos.get();
        let rect = area.move_(-pos.x1(), -pos.y1());
        let mut rect = self.mapped.connector_damage_matrix.get().apply(0, 0, rect);
        let damage = &mut *self.global.connector.damage.borrow_mut();
        const MAX_CONNECTOR_DAMAGE: usize = 32;
        if damage.len() >= MAX_CONNECTOR_DAMAGE {
            rect = rect.union(damage.pop().unwrap());
        }
        damage.push(rect.intersect(self.mapped.connector_damage_intersect.get()));
    }

    pub fn add_visualizer_damage(&self) {
        self.state.damage_visualizer.copy_damage(self);
    }

    fn update_color_description_(self: &Rc<Self>, tt: &TreeTransaction) -> bool {
        let (mut luminance, tf) = match self.current.btf.get() {
            BackendEotfs::Default => (Luminance::SRGB, Eotf::Gamma22),
            BackendEotfs::Pq => (Luminance::ST2084_PQ, Eotf::St2084Pq),
        };
        if let Some(brightness) = self.current.brightness.get() {
            luminance.white.0 = brightness;
        }
        let mut target_luminance = luminance.to_target();
        let mut max_cll = None;
        let mut max_fall = None;
        if let Some(l) = self.global.luminance
            && self.current.btf.get() == BackendEotfs::Pq
        {
            target_luminance.min = F64(l.min);
            target_luminance.max = F64(l.max);
            max_cll = Some(F64(l.max));
            max_fall = Some(F64(l.max_fall));
        }
        let primaries = match self.current.bcs.get() {
            BackendColorSpace::Default => NamedPrimaries::Srgb,
            BackendColorSpace::Bt2020 => NamedPrimaries::Bt2020,
        };
        let cd = self.state.color_manager.get_description(
            Some(primaries),
            primaries.primaries(),
            luminance,
            tf,
            self.global.primaries,
            target_luminance,
            max_cll,
            max_fall,
        );
        let cd_linear = self.state.color_manager.get_with_tf(&cd, Eotf::Linear);
        self.set_current_color_description(tt, &cd, &cd_linear).id != cd.id
    }

    fn add_op(self: &Rc<Self>, tt: &TreeTransaction, kind: OutputTreeOpKind) {
        tt.add_op(
            &self.timeline,
            OutputTreeOp {
                on: self.clone(),
                kind,
            },
        );
    }

    pub fn damage_if_visible(self: &Rc<Self>, tt: &TreeTransaction) {
        self.add_op(tt, DamageIfVisible);
    }

    fn set_current_pos(self: &Rc<Self>, tt: &TreeTransaction, pos: Rect) {
        self.current.pos.set(pos);
        self.add_op(tt, SetPos(pos));
    }

    fn set_current_transform(self: &Rc<Self>, tt: &TreeTransaction, transform: Transform) {
        self.current.transform.set(transform);
        self.add_op(tt, SetTransform(transform));
    }

    fn set_current_scale(self: &Rc<Self>, tt: &TreeTransaction, scale: Scale) -> Scale {
        let old = self.current.scale.replace(scale);
        self.add_op(tt, SetScale(scale));
        old
    }

    fn set_current_legacy_scale(self: &Rc<Self>, tt: &TreeTransaction, scale: u32) -> u32 {
        let old = self.current.legacy_scale.replace(scale);
        self.add_op(tt, SetLegacyScale(scale));
        old
    }

    pub fn set_current_workspace(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        workspace: Option<&Rc<WorkspaceNode>>,
    ) -> Option<Rc<WorkspaceNode>> {
        let old = self.current.workspace.set(workspace.cloned());
        self.add_op(tt, SetWorkspace(workspace.cloned()));
        old
    }

    pub fn set_current_lock_surface(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        ls: Option<&Rc<ExtSessionLockSurfaceV1>>,
    ) -> Option<Rc<ExtSessionLockSurfaceV1>> {
        let old = self.current.lock_surface.set(ls.cloned());
        self.add_op(tt, SetLockSurface(ls.cloned()));
        old
    }

    fn set_current_non_exclusive_rect(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        ner: Rect,
        nerr: Rect,
    ) {
        self.current.non_exclusive_rect.set(ner);
        self.current.non_exclusive_rect_rel.set(nerr);
        self.add_op(tt, SetNonExclusiveRect(ner, nerr));
    }

    pub fn set_current_brightness(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        v: Option<f64>,
    ) -> Option<f64> {
        let old = self.current.brightness.replace(v);
        self.add_op(tt, SetBrightness(v));
        old
    }

    pub fn set_current_blend_space(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        v: BlendSpace,
    ) -> BlendSpace {
        let old = self.current.blend_space.replace(v);
        self.add_op(tt, SetBlendSpace(v));
        old
    }

    pub fn set_current_btf(self: &Rc<Self>, tt: &TreeTransaction, v: BackendEotfs) -> BackendEotfs {
        let old = self.current.btf.replace(v);
        self.add_op(tt, SetBtf(v));
        old
    }

    pub fn set_current_bcs(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        v: BackendColorSpace,
    ) -> BackendColorSpace {
        let old = self.current.bcs.replace(v);
        self.add_op(tt, SetBcs(v));
        old
    }

    pub fn set_current_color_description(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        cd: &Rc<ColorDescription>,
        linear_cd: &Rc<ColorDescription>,
    ) -> Rc<ColorDescription> {
        self.current.linear_color_description.set(linear_cd.clone());
        let old = self.current.color_description.set(cd.clone());
        self.add_op(tt, SetColorDescription(cd.clone(), linear_cd.clone()));
        old
    }

    pub fn set_current_damage_matrix(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        damage_matrix: DamageMatrix,
        damage_intersect: Rect,
    ) {
        self.current.connector_damage_matrix.set(damage_matrix);
        self.current
            .connector_damage_intersect
            .set(damage_intersect);
        self.add_op(tt, SetDamageMatrix(damage_matrix, damage_intersect));
    }
}

pub struct OutputTitle {
    pub x1: i32,
    pub x2: i32,
    pub icon_x: Option<i32>,
    pub tex_x: i32,
    pub tex_y: i32,
    pub tex: Rc<dyn GfxTexture>,
    pub ws: Rc<WorkspaceNode>,
}

pub struct OutputStatus {
    pub tex_x: i32,
    pub tex: TextTexture,
}

#[derive(Copy, Clone)]
pub struct OutputWorkspaceRenderData {
    pub rect: Rect,
    pub captured: bool,
}

#[derive(Default)]
pub struct OutputRenderData {
    pub full_area: Rect,
    pub active_workspace: Option<OutputWorkspaceRenderData>,
    pub overlay_workspace: Option<Rect>,
    pub bar_separator: Rect,
    pub inactive_workspaces: Vec<Rect>,
    pub attention_requested_workspaces: Vec<Rect>,
    pub captured_inactive_workspaces: Vec<Rect>,
    pub titles: Vec<OutputTitle>,
    pub status: Option<OutputStatus>,
}

impl OutputRenderData {
    fn clear(&mut self) {
        self.titles.clear();
        self.status.take();
    }
}

impl Debug for OutputNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputNode").finish_non_exhaustive()
    }
}

impl Node for OutputNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_output(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(ls) = self.current.lock_surface.get() {
            visitor.visit_lock_surface(&ls);
        }
        for ws in self.current_workspaces() {
            visitor.visit_workspace(&ws);
        }
        if let Some(ws) = self.current.overlay.get() {
            visitor.visit_workspace(&ws);
        }
        for layers in &self.layers {
            for surface in layers.iter() {
                visitor.visit_layer_surface(surface.deref());
            }
        }
        for item in self.tray_items.iter() {
            item.deref().clone().node_visit(visitor);
        }
    }

    fn node_visible(&self) -> bool {
        self.state.root_visible()
    }

    fn node_mapped_position(&self) -> Rect {
        self.current.pos.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        self.global.opt.node()
    }

    fn node_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        None
    }

    fn node_location(&self) -> Option<NodeLocation> {
        Some(NodeLocation::Output(self.id))
    }

    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Output
    }

    fn node_do_focus(
        self: Rc<Self>,
        tt: &TreeTransaction,
        seat: &Rc<WlSeatGlobal>,
        direction: Direction,
    ) {
        if self.state.lock.locked.get() {
            if let Some(lock) = self.current.lock_surface.get() {
                seat.focus_node(lock.surface.clone());
            }
            return;
        }
        if let Some(ws) = self.workspace() {
            ws.do_focus(tt, seat, direction);
        }
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        if self.state.lock.locked.get() {
            let allow_surface = match usecase {
                FindTreeUsecase::None => true,
                FindTreeUsecase::SelectToplevel => false,
                FindTreeUsecase::SelectToplevelOrPopup => false,
                FindTreeUsecase::SelectNormalWorkspace => false,
            };
            if allow_surface && let Some(ls) = self.current.lock_surface.get() {
                tree.push(FoundNode {
                    node: ls.clone(),
                    x,
                    y,
                });
                return ls.node_find_tree_at(x, y, tree, usecase);
            }
            return FindTreeResult::AcceptsInput;
        }
        let ws_rect_rel = self.current.workspace_rect_rel.get();
        let select_workspace = match usecase {
            FindTreeUsecase::None => false,
            FindTreeUsecase::SelectToplevel => false,
            FindTreeUsecase::SelectToplevelOrPopup => false,
            FindTreeUsecase::SelectNormalWorkspace => true,
        };
        if select_workspace && ws_rect_rel.contains(x, y) {
            let (x, y) = ws_rect_rel.translate(x, y);
            if let Some(ws) = self.current.workspace.get() {
                tree.push(FoundNode {
                    node: ws.clone(),
                    x,
                    y,
                });
                return FindTreeResult::AcceptsInput;
            }
        }
        {
            let res =
                self.find_stacked_at(&self.state.root.stacked_in_overlay, x, y, tree, usecase);
            if res.accepts_input() {
                return res;
            }
        }
        if let Some(ws) = self.current.overlay.get() {
            if let Some(fs) = ws.current.fullscreen.get() {
                tree.push(FoundNode {
                    node: fs.clone(),
                    x,
                    y,
                });
                return fs.node_find_tree_at(x, y, tree, usecase);
            }
            if ws_rect_rel.contains(x, y) {
                let (x, y) = ws_rect_rel.translate(x, y);
                let len = tree.len();
                tree.push(FoundNode {
                    node: ws.clone(),
                    x,
                    y,
                });
                let res = ws.node_find_tree_at(x, y, tree, usecase);
                if res.accepts_input() {
                    return res;
                }
                tree.truncate(len);
            }
        }
        {
            let res =
                self.find_stacked_at(&self.state.root.stacked_above_layers, x, y, tree, usecase);
            if res.accepts_input() {
                return res;
            }
        }
        let mut fullscreen = None;
        if let Some(ws) = self.mapped.workspace.get() {
            fullscreen = ws.mapped.fullscreen.get();
        }
        {
            let mut layers = &[OVERLAY, TOP][..];
            if fullscreen.is_some() {
                layers = &[OVERLAY];
            }
            let res = self.find_layer_surface_at(x, y, layers, tree, usecase);
            if res.accepts_input() {
                return res;
            }
        }
        {
            let res = self.find_stacked_at(&self.state.root.stacked, x, y, tree, usecase);
            if res.accepts_input() {
                return res;
            }
        }
        if let Some(fs) = fullscreen {
            tree.push(FoundNode {
                node: fs.clone(),
                x,
                y,
            });
            fs.node_find_tree_at(x, y, tree, usecase)
        } else {
            let mut search_layers = true;
            let bar_rect_rel = self.current.bar_rect_rel.get();
            if bar_rect_rel.contains(x, y) {
                let (x, y) = bar_rect_rel.translate(x, y);
                search_layers = false;
                for item in self.tray_items.iter() {
                    let data = item.data();
                    let pos = data.rel_pos.get();
                    if pos.contains(x, y) {
                        let (x, y) = pos.translate(x, y);
                        tree.push(FoundNode {
                            node: item.deref().clone(),
                            x,
                            y,
                        });
                        return data.find_tree_at(x, y, tree);
                    }
                }
            } else if ws_rect_rel.contains(x, y)
                && let Some(ws) = self.current.workspace.get()
            {
                let (x, y) = ws_rect_rel.translate(x, y);
                let len = tree.len();
                tree.push(FoundNode {
                    node: ws.clone(),
                    x,
                    y,
                });
                match ws.node_find_tree_at(x, y, tree, usecase) {
                    FindTreeResult::AcceptsInput => search_layers = false,
                    FindTreeResult::Other => {
                        tree.truncate(len);
                    }
                }
            }
            if search_layers {
                self.find_layer_surface_at(x, y, &[BOTTOM, BACKGROUND], tree, usecase);
            }
            FindTreeResult::AcceptsInput
        }
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, _bounds: Option<&Rect>) {
        renderer.render_output(self, x, y);
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        _time_usec: u64,
        button: u32,
        state: ButtonState,
        _serial: u64,
    ) {
        if button != BTN_LEFT && button != BTN_MIDDLE {
            return;
        }
        if state != ButtonState::Pressed {
            self.pointer_down.remove(&seat.id());
            return;
        }
        self.button(seat, PointerType::Seat(seat.id()), button);
    }

    fn node_on_axis_event(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, event: &PendingScroll) {
        let steps = match self.scroll.handle(event) {
            Some(e) => e,
            _ => return,
        };
        if steps == 0 {
            return;
        }
        let ws = match self.current.workspace.get() {
            Some(ws) => ws,
            _ => return,
        };
        let Some(ws) = ws.current.output_link.get() else {
            return;
        };
        let mut ws = ws.to_ref();
        let next = |ws: &NodeRef<WorkspaceInOutput>| {
            if steps < 0 { ws.prev() } else { ws.next() }
        };
        for _ in 0..steps.abs() {
            let mut new = next(&ws);
            while let Some(n) = &new {
                if n.is_current_link.get() {
                    break;
                }
                new = next(n);
            }
            ws = match new {
                Some(n) => n,
                None => break,
            };
        }
        let tt = &self.state.tree_transaction();
        self.state.show_workspace2(tt, Some(seat), &self, &ws);
    }

    fn node_on_leave(&self, seat: &WlSeatGlobal) {
        self.pointer_down.remove(&seat.id());
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(PointerType::Seat(seat.id()), x, y);
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("output focus");
        seat.pointer_cursor().set_known(KnownCursor::Default);
    }

    fn node_on_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(PointerType::Seat(seat.id()), x, y);
        if let Some((down_x, down_y)) = self.pointer_down.get(&seat.id())
            && self
                .state
                .ui_drag_threshold_reached((x.round_down(), y.round_down()), (down_x, down_y))
        {
            let bar_rect_rel = self.current.bar_rect_rel.get();
            if bar_rect_rel.contains(down_x, down_y) {
                let (down_x, _) = bar_rect_rel.translate(down_x, down_y);
                let rd = self.render_data.borrow_mut();
                for title in &rd.titles {
                    if down_x >= title.x1 && down_x < title.x2 {
                        let ws = title.ws.clone();
                        drop(rd);
                        seat.start_workspace_drag(&ws);
                        break;
                    }
                }
            }
        }
    }

    fn node_on_tablet_tool_leave(&self, tool: &Rc<TabletTool>, _time_usec: u64) {
        self.pointer_positions
            .remove(&PointerType::TabletTool(tool.id));
    }

    fn node_on_tablet_tool_enter(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        _time_usec: u64,
        x: Fixed,
        y: Fixed,
    ) {
        tool.cursor().set_known(KnownCursor::Default);
        self.pointer_move(PointerType::TabletTool(tool.id), x, y);
    }

    fn node_on_tablet_tool_apply_changes(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        _time_usec: u64,
        changes: Option<&TabletToolChanges>,
        x: Fixed,
        y: Fixed,
    ) {
        let id = PointerType::TabletTool(tool.id);
        self.pointer_move(id, x, y);
        if let Some(changes) = changes {
            if changes.down == Some(true) {
                self.button(tool.seat(), id, BTN_LEFT);
            }
        }
    }
}

pub fn calculate_logical_size(
    mode: (i32, i32),
    transform: Transform,
    scale: crate::scale::Scale,
) -> (i32, i32) {
    let (mut width, mut height) = transform.maybe_swap(mode);
    if scale != 1 {
        let scale = scale.to_f64();
        width = (width as f64 / scale).round() as _;
        height = (height as f64 / scale).round() as _;
    }
    (width, height)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum VrrMode {
    #[default]
    Never,
    Always,
    Fullscreen {
        surface: Option<VrrSurfaceRequirements>,
    },
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct VrrSurfaceRequirements {
    pub content_type: Option<VrrContentTypeRequirements>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct VrrContentTypeRequirements {
    pub photo: bool,
    pub video: bool,
    pub game: bool,
}

impl Default for VrrContentTypeRequirements {
    fn default() -> Self {
        Self {
            photo: true,
            video: true,
            game: true,
        }
    }
}

impl VrrMode {
    pub const NEVER: &'static Self = &Self::Never;
    pub const ALWAYS: &'static Self = &Self::Always;
    pub const VARIANT_1: &'static Self = &Self::Fullscreen { surface: None };
    pub const VARIANT_2: &'static Self = &Self::Fullscreen {
        surface: Some(VrrSurfaceRequirements { content_type: None }),
    };
    pub const VARIANT_3: &'static Self = &Self::Fullscreen {
        surface: Some(VrrSurfaceRequirements {
            content_type: Some(VrrContentTypeRequirements {
                photo: false,
                video: true,
                game: true,
            }),
        }),
    };

    pub fn from_config(mode: ConfigVrrMode) -> Option<&'static Self> {
        let res = match mode {
            ConfigVrrMode::NEVER => Self::NEVER,
            ConfigVrrMode::ALWAYS => Self::ALWAYS,
            ConfigVrrMode::VARIANT_1 => Self::VARIANT_1,
            ConfigVrrMode::VARIANT_2 => Self::VARIANT_2,
            ConfigVrrMode::VARIANT_3 => Self::VARIANT_3,
            _ => return None,
        };
        Some(res)
    }

    pub fn to_config(&self) -> ConfigVrrMode {
        match self {
            Self::NEVER => ConfigVrrMode::NEVER,
            Self::ALWAYS => ConfigVrrMode::ALWAYS,
            Self::VARIANT_1 => ConfigVrrMode::VARIANT_1,
            Self::VARIANT_2 => ConfigVrrMode::VARIANT_2,
            Self::VARIANT_3 => ConfigVrrMode::VARIANT_3,
            _ => {
                log::error!("VRR mode {self:?} has no config representation");
                ConfigVrrMode::NEVER
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum TearingMode {
    #[default]
    Never,
    Always,
    Fullscreen {
        surface: Option<TearingSurfaceRequirements>,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TearingSurfaceRequirements {
    pub tearing_requested: bool,
}

impl Default for TearingSurfaceRequirements {
    fn default() -> Self {
        Self {
            tearing_requested: true,
        }
    }
}

impl TearingMode {
    pub const NEVER: &'static Self = &Self::Never;
    pub const ALWAYS: &'static Self = &Self::Always;
    pub const VARIANT_1: &'static Self = &Self::Fullscreen { surface: None };
    pub const VARIANT_2: &'static Self = &Self::Fullscreen {
        surface: Some(TearingSurfaceRequirements {
            tearing_requested: false,
        }),
    };
    pub const VARIANT_3: &'static Self = &Self::Fullscreen {
        surface: Some(TearingSurfaceRequirements {
            tearing_requested: true,
        }),
    };

    pub fn from_config(mode: ConfigTearingMode) -> Option<&'static Self> {
        let res = match mode {
            ConfigTearingMode::NEVER => Self::NEVER,
            ConfigTearingMode::ALWAYS => Self::ALWAYS,
            ConfigTearingMode::VARIANT_1 => Self::VARIANT_1,
            ConfigTearingMode::VARIANT_2 => Self::VARIANT_2,
            ConfigTearingMode::VARIANT_3 => Self::VARIANT_3,
            _ => return None,
        };
        Some(res)
    }

    pub fn to_config(&self) -> ConfigTearingMode {
        match self {
            Self::NEVER => ConfigTearingMode::NEVER,
            Self::ALWAYS => ConfigTearingMode::ALWAYS,
            Self::VARIANT_1 => ConfigTearingMode::VARIANT_1,
            Self::VARIANT_2 => ConfigTearingMode::VARIANT_2,
            Self::VARIANT_3 => ConfigTearingMode::VARIANT_3,
        }
    }
}

pub enum OutputNodeOrPersistent {
    Node(Rc<OutputNode>),
    Persistent(Rc<PersistentOutputState>),
}

impl OutputNodeOrPersistent {
    pub fn set_position(&self, tt: &TreeTransaction, x: i32, y: i32) {
        match self {
            OutputNodeOrPersistent::Node(n) => n.set_position(tt, x, y),
            OutputNodeOrPersistent::Persistent(p) => p.pos.set((x, y)),
        }
    }

    pub fn set_preferred_scale(&self, tt: &TreeTransaction, scale: Scale) {
        match self {
            OutputNodeOrPersistent::Node(n) => n.set_preferred_scale(tt, scale),
            OutputNodeOrPersistent::Persistent(p) => p.scale.set(scale),
        }
    }

    pub fn update_transform(&self, tt: &TreeTransaction, transform: Transform) {
        match self {
            OutputNodeOrPersistent::Node(n) => n.update_transform(tt, transform),
            OutputNodeOrPersistent::Persistent(p) => p.transform.set(transform),
        }
    }

    pub fn set_vrr_mode(&self, tt: &TreeTransaction, mode: &VrrMode) {
        match self {
            OutputNodeOrPersistent::Node(n) => n.set_vrr_mode(tt, mode),
            OutputNodeOrPersistent::Persistent(p) => p.vrr_mode.set(*mode),
        }
    }

    pub fn set_tearing_mode(&self, tt: &TreeTransaction, mode: &TearingMode) {
        match self {
            OutputNodeOrPersistent::Node(n) => n.set_tearing_mode(tt, mode),
            OutputNodeOrPersistent::Persistent(p) => p.tearing_mode.set(*mode),
        }
    }

    pub fn set_brightness(&self, tt: &TreeTransaction, brightness: Option<f64>) {
        match self {
            OutputNodeOrPersistent::Node(n) => n.set_brightness(tt, brightness),
            OutputNodeOrPersistent::Persistent(p) => p.brightness.set(brightness),
        }
    }

    pub fn set_blend_space(&self, tt: &TreeTransaction, blend_space: BlendSpace) {
        match self {
            OutputNodeOrPersistent::Node(n) => n.set_blend_space(tt, blend_space),
            OutputNodeOrPersistent::Persistent(p) => p.blend_space.set(blend_space),
        }
    }

    pub fn set_use_native_gamut(&self, tt: &TreeTransaction, use_native_gamut: bool) {
        match self {
            OutputNodeOrPersistent::Node(n) => n.set_use_native_gamut(tt, use_native_gamut),
            OutputNodeOrPersistent::Persistent(p) => p.use_native_gamut.set(use_native_gamut),
        }
    }

    pub fn set_cursor_hz(&self, state: &State, hz: Option<f64>) {
        match self {
            OutputNodeOrPersistent::Node(n) => {
                n.schedule.set_cursor_hz(state, hz.unwrap_or(f64::INFINITY))
            }
            OutputNodeOrPersistent::Persistent(p) => p.vrr_cursor_hz.set(hz),
        }
    }
}

impl PartialEq for OutputNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub struct OutputTreeOp {
    on: Rc<OutputNode>,
    kind: OutputTreeOpKind,
}

enum OutputTreeOpKind {
    SetPos(Rect),
    SetTransform(Transform),
    SetScale(Scale),
    SetLegacyScale(u32),
    SetWorkspace(Option<Rc<WorkspaceNode>>),
    SetLockSurface(Option<Rc<ExtSessionLockSurfaceV1>>),
    SetNonExclusiveRect(Rect, Rect),
    SetBrightness(Option<f64>),
    SetBlendSpace(BlendSpace),
    SetBtf(BackendEotfs),
    SetBcs(BackendColorSpace),
    SetColorDescription(Rc<ColorDescription>, Rc<ColorDescription>),
    SetDamageMatrix(DamageMatrix, Rect),
    DamageIfVisible,
    DamageHardwareCursor,
    ScheduleUpdateRenderData,
}

impl TreeTransactionOp for OutputTreeOp {
    fn unblocked(self, _serial: TreeSerial, _timeout: bool) {
        let on = self.on;
        let m = &on.mapped;
        match self.kind {
            SetPos(v) => {
                m.pos.set(v);
            }
            SetTransform(v) => {
                m.transform.set(v);
            }
            SetScale(v) => {
                m.scale.set(v);
            }
            SetLegacyScale(v) => {
                m.legacy_scale.set(v);
            }
            SetWorkspace(v) => {
                m.workspace.set(v);
            }
            SetLockSurface(v) => {
                m.lock_surface.set(v);
            }
            SetNonExclusiveRect(ner, nerr) => {
                m.non_exclusive_rect.set(ner);
                m.non_exclusive_rect_rel.set(nerr);
            }
            SetBrightness(v) => {
                m.brightness.set(v);
            }
            SetBlendSpace(v) => {
                m.blend_space.set(v);
            }
            SetBcs(v) => {
                m.bcs.set(v);
            }
            SetBtf(v) => {
                m.btf.set(v);
            }
            SetColorDescription(cd, linear_cd) => {
                m.color_description.set(cd);
                m.linear_color_description.set(linear_cd);
            }
            SetDamageMatrix(damage_matrix, damage_intersect) => {
                m.connector_damage_matrix.set(damage_matrix);
                m.connector_damage_intersect.set(damage_intersect);
            }
            DamageIfVisible => {
                if on.node_visible() {
                    on.state.damage(m.pos.get());
                }
            }
            DamageHardwareCursor => {
                if let Some(hc) = on.hardware_cursor.get() {
                    on.hardware_cursor_needs_render.set(true);
                    hc.damage();
                }
            }
            ScheduleUpdateRenderData => {
                if !on.update_render_data_scheduled.replace(true) {
                    on.state.pending_output_render_data.push(on.clone());
                }
            }
        }
    }
}
