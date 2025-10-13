pub mod commit_timeline;
pub mod cursor;
pub mod dnd_icon;
pub mod ext_session_lock_surface_v1;
pub mod tray;
pub mod wl_subsurface;
pub mod wp_alpha_modifier_surface_v1;
pub mod wp_color_management_surface_v1;
pub mod wp_commit_timer_v1;
pub mod wp_fifo_v1;
pub mod wp_fractional_scale_v1;
pub mod wp_linux_drm_syncobj_surface_v1;
pub mod wp_tearing_control_v1;
pub mod wp_viewport;
pub mod x_surface;
pub mod xdg_surface;
pub mod xwayland_shell_v1;
pub mod zwlr_layer_surface_v1;
pub mod zwp_idle_inhibitor_v1;
pub mod zwp_input_popup_surface_v2;

use {
    crate::{
        backend::KeyState,
        client::{Client, ClientError},
        cmm::cmm_description::ColorDescription,
        configurable::ConfigurableDataCore,
        cursor_user::{CursorUser, CursorUserId},
        damage::DamageMatrix,
        drm_feedback::DrmFeedback,
        fixed::Fixed,
        gfx_api::{
            AsyncShmGfxTexture, BufferResv, BufferResvUser, GfxError, GfxStagingBuffer,
            ReleaseSync, SampleRect, SyncFile,
        },
        ifs::{
            color_management::wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1,
            wl_buffer::WlBuffer,
            wl_callback::WlCallback,
            wl_seat::{
                Dnd, NodeSeatState, SeatId, WlSeatGlobal,
                tablet::{
                    PadButtonState, TabletPad, TabletPadDial, TabletPadGroup, TabletPadRing,
                    TabletPadStrip, TabletRingEventSource, TabletStripEventSource, TabletTool,
                    TabletToolChanges, ToolButtonState,
                },
                text_input::TextInputConnection,
                wl_pointer::PendingScroll,
                zwp_pointer_constraints_v1::SeatConstraint,
            },
            wl_surface::{
                commit_timeline::{ClearReason, CommitTimeline, CommitTimelineError},
                cursor::CursorSurface,
                dnd_icon::DndIcon,
                tray::TrayItemId,
                wl_subsurface::{PendingSubsurfaceData, SubsurfaceId, WlSubsurface},
                wp_alpha_modifier_surface_v1::WpAlphaModifierSurfaceV1,
                wp_commit_timer_v1::WpCommitTimerV1,
                wp_fifo_v1::WpFifoV1,
                wp_fractional_scale_v1::WpFractionalScaleV1,
                wp_linux_drm_syncobj_surface_v1::WpLinuxDrmSyncobjSurfaceV1,
                wp_tearing_control_v1::WpTearingControlV1,
                wp_viewport::WpViewport,
                x_surface::{XSurface, xwindow::Xwindow},
                xdg_surface::{PendingXdgSurfaceData, XdgSurfaceError, xdg_toplevel::XdgToplevel},
                zwlr_layer_surface_v1::{PendingLayerSurfaceData, ZwlrLayerSurfaceV1Error},
            },
            wp_content_type_v1::ContentType,
            wp_presentation_feedback::{VRR_REFRESH_SINCE, WpPresentationFeedback},
            zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
        },
        io_uring::IoUringError,
        keyboard::KeyboardState,
        leaks::Tracker,
        object::{Object, Version},
        rect::{DamageQueue, Rect, Region},
        renderer::Renderer,
        tree::{
            BeforeLatchListener, BeforeLatchResult, ContainerNode, FindTreeResult, FoundNode,
            LatchListener, Node, NodeId, NodeLayerLink, NodeLocation, NodeVisitor, NodeVisitorBase,
            OutputNode, PlaceholderNode, PresentationListener, ToplevelNode, TreeSerial,
            VblankListener,
            transaction::{
                TreeBarrier, TreeTransaction, TreeTransactionOp, TreeTransactionTimeline,
            },
        },
        utils::{
            cell_ext::CellExt, clonecell::CloneCell, copyhashmap::CopyHashMap,
            double_buffered::DoubleBuffered, errorfmt::ErrorFmt, event_listener::EventListener,
            linkedlist::LinkedList, numcell::NumCell, smallmap::SmallMap, syncqueue::SyncQueue,
            transform_ext::TransformExt,
        },
        video::{
            dmabuf::DMA_BUF_SYNC_READ,
            drm::sync_obj::{SyncObj, SyncObjPoint},
        },
        wire::{
            WlOutputId, WlSurfaceId, WpColorManagementSurfaceFeedbackV1Id, ZwpIdleInhibitorV1Id,
            ZwpLinuxDmabufFeedbackV1Id, wl_surface::*,
        },
        xwayland::XWaylandEvent,
    },
    ahash::AHashMap,
    isnt::std_1::{primitive::IsntSliceExt, vec::IsntVecExt},
    jay_config::video::Transform,
    std::{
        cell::{Cell, RefCell},
        collections::hash_map::{Entry, OccupiedEntry},
        fmt::{Debug, Formatter},
        mem,
        ops::{Deref, DerefMut},
        rc::{Rc, Weak},
    },
    thiserror::Error,
    wp_color_management_surface_v1::WpColorManagementSurfaceV1,
    zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1,
};

#[expect(dead_code)]
const INVALID_SCALE: u32 = 0;
#[expect(dead_code)]
const INVALID_TRANSFORM: u32 = 1;
#[expect(dead_code)]
const INVALID_SIZE: u32 = 2;

const OFFSET_SINCE: Version = Version(5);
const BUFFER_SCALE_SINCE: Version = Version(6);
const TRANSFORM_SINCE: Version = Version(6);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SurfaceRole {
    None,
    Subsurface,
    XdgSurface,
    Cursor,
    DndIcon,
    ZwlrLayerSurface,
    XSurface,
    ExtSessionLockSurface,
    InputPopup,
    TrayItem,
}

impl SurfaceRole {
    fn name(self) -> &'static str {
        match self {
            SurfaceRole::None => "none",
            SurfaceRole::Subsurface => "subsurface",
            SurfaceRole::XdgSurface => "xdg_surface",
            SurfaceRole::Cursor => "cursor",
            SurfaceRole::DndIcon => "dnd_icon",
            SurfaceRole::ZwlrLayerSurface => "zwlr_layer_surface",
            SurfaceRole::XSurface => "xwayland surface",
            SurfaceRole::ExtSessionLockSurface => "ext_session_lock_surface",
            SurfaceRole::InputPopup => "input_popup_surface",
            SurfaceRole::TrayItem => "tray_item",
        }
    }
}

pub struct SurfaceSendPreferredScaleVisitor;

impl SurfaceSendPreferredScaleVisitor {
    fn schedule_realloc(&self, tl: &impl ToplevelNode) {
        let data = tl.tl_data();
        for sc in data.jay_screencasts.lock().values() {
            sc.schedule_realloc_or_reconfigure();
        }
        for sc in data.ext_copy_sessions.lock().values() {
            sc.buffer_size_changed();
        }
    }
}

impl NodeVisitorBase for SurfaceSendPreferredScaleVisitor {
    fn visit_surface(&mut self, node: &Rc<WlSurface>) {
        node.on_scale_change();
        node.node_visit_children(self);
    }

    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
        self.schedule_realloc(&**node);
        node.node_visit_children(self);
    }

    fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
        self.schedule_realloc(&**node);
        node.node_visit_children(self);
    }

    fn visit_container(&mut self, node: &Rc<ContainerNode>) {
        self.schedule_realloc(&**node);
        node.node_visit_children(self);
    }

    fn visit_placeholder(&mut self, node: &Rc<PlaceholderNode>) {
        self.schedule_realloc(&**node);
        node.node_visit_children(self);
    }
}

pub struct SurfaceSendPreferredTransformVisitor;
impl NodeVisitorBase for SurfaceSendPreferredTransformVisitor {
    fn visit_surface(&mut self, node: &Rc<WlSurface>) {
        node.send_preferred_buffer_transform();
        node.node_visit_children(self);
    }
}

pub struct SurfaceSendPreferredColorDescription;
impl NodeVisitorBase for SurfaceSendPreferredColorDescription {
    fn visit_surface(&mut self, node: &Rc<WlSurface>) {
        node.send_preferred_color_description();
        node.node_visit_children(self);
    }
}

struct SurfaceBufferExplicitRelease {
    sync_obj: Rc<SyncObj>,
    point: SyncObjPoint,
}

pub struct SurfaceBuffer {
    pub buffer: Rc<WlBuffer>,
    sync_files: SmallMap<BufferResvUser, SyncFile, 1>,
    pub release_sync: ReleaseSync,
    release: Option<SurfaceBufferExplicitRelease>,
}

impl Drop for SurfaceBuffer {
    fn drop(&mut self) {
        let sync_files = self.sync_files.take();
        if let Some(release) = &self.release {
            let Some(ctx) = self.buffer.client.state.render_ctx.get() else {
                log::error!("Cannot signal release point because there is no render context");
                return;
            };
            let Some(ctx) = ctx.sync_obj_ctx() else {
                log::error!("Cannot signal release point because there is no syncobj context");
                return;
            };
            if sync_files.is_not_empty() {
                let res = ctx.import_sync_files(
                    &release.sync_obj,
                    release.point,
                    sync_files.iter().map(|f| &f.1),
                );
                match res {
                    Ok(_) => return,
                    Err(e) => {
                        log::error!("Could not import sync files into sync obj: {}", ErrorFmt(e));
                    }
                }
            }
            if let Err(e) = ctx.signal(&release.sync_obj, release.point) {
                log::error!("Could not signal release point: {}", ErrorFmt(e));
            }
            return;
        }
        if let Some(dmabuf) = &self.buffer.dmabuf {
            for (_, sync_file) in &sync_files {
                if let Err(e) = dmabuf.import_sync_file(DMA_BUF_SYNC_READ, sync_file) {
                    log::error!("Could not import sync file: {}", ErrorFmt(e));
                }
            }
        }
        if !self.buffer.destroyed() {
            self.buffer.send_release();
        }
    }
}

impl Debug for SurfaceBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceBuffer").finish_non_exhaustive()
    }
}

impl BufferResv for SurfaceBuffer {
    fn set_sync_file(&self, user: BufferResvUser, sync_file: &SyncFile) {
        self.sync_files.insert(user, sync_file.clone());
    }
}

pub struct SurfaceShmTexture {
    pub tex: CloneCell<Option<Rc<dyn AsyncShmGfxTexture>>>,
    pub damage: DamageQueue,
}

pub struct WlSurface {
    pub id: WlSurfaceId,
    pub node_id: SurfaceNodeId,
    pub client: Rc<Client>,
    visible: Cell<bool>,
    role: Cell<SurfaceRole>,
    pending: RefCell<Box<PendingState>>,
    input_region: CloneCell<Option<Rc<Region>>>,
    opaque_region: CloneCell<Option<Rc<Region>>>,
    buffer_points: RefCell<BufferPoints>,
    pub buffer_points_norm: RefCell<SampleRect>,
    damage_matrix: Cell<DamageMatrix>,
    buffer_transform: Cell<Transform>,
    buffer_scale: Cell<i32>,
    src_rect: Cell<Option<[Fixed; 4]>>,
    dst_size: Cell<Option<(i32, i32)>>,
    pub extents: Cell<Rect>,
    pub buffer_abs_pos: Cell<Rect>,
    pub need_extents_update: Cell<bool>,
    need_extents_propagation: Cell<bool>,
    pub buffer: CloneCell<Option<Rc<SurfaceBuffer>>>,
    pub shm_staging: CloneCell<Option<Rc<dyn GfxStagingBuffer>>>,
    pub shm_textures: DoubleBuffered<SurfaceShmTexture>,
    pub buf_x: NumCell<i32>,
    pub buf_y: NumCell<i32>,
    pub children: RefCell<Option<Box<ParentData>>>,
    ext: CloneCell<Rc<dyn SurfaceExt>>,
    frame_requests: RefCell<Vec<Rc<WlCallback>>>,
    presentation_feedback: RefCell<Vec<Rc<WpPresentationFeedback>>>,
    latched_presentation_feedback: RefCell<Vec<Rc<WpPresentationFeedback>>>,
    seat_state: NodeSeatState,
    toplevel: CloneCell<Option<Rc<dyn ToplevelNode>>>,
    cursors: SmallMap<CursorUserId, Rc<CursorSurface>, 1>,
    dnd_icons: SmallMap<SeatId, Rc<DndIcon>, 1>,
    pub tracker: Tracker<Self>,
    idle_inhibitors: SmallMap<ZwpIdleInhibitorV1Id, Rc<ZwpIdleInhibitorV1>, 1>,
    viewporter: CloneCell<Option<Rc<WpViewport>>>,
    output: CloneCell<Rc<OutputNode>>,
    location: Cell<NodeLocation>,
    fractional_scale: CloneCell<Option<Rc<WpFractionalScaleV1>>>,
    pub constraints: SmallMap<SeatId, Rc<SeatConstraint>, 1>,
    xwayland_serial: Cell<Option<u64>>,
    tearing_control: CloneCell<Option<Rc<WpTearingControlV1>>>,
    pub tearing: Cell<bool>,
    version: Version,
    pub has_content_type_manager: Cell<bool>,
    pub content_type: Cell<Option<ContentType>>,
    pub drm_feedback: CopyHashMap<ZwpLinuxDmabufFeedbackV1Id, Rc<ZwpLinuxDmabufFeedbackV1>>,
    sync_obj_surface: CloneCell<Option<Rc<WpLinuxDrmSyncobjSurfaceV1>>>,
    destroyed: Cell<bool>,
    commit_timeline: CommitTimeline,
    alpha_modifier: CloneCell<Option<Rc<WpAlphaModifierSurfaceV1>>>,
    alpha: Cell<Option<f32>>,
    pub text_input_connections: SmallMap<SeatId, Rc<TextInputConnection>, 1>,
    vblank_listener: EventListener<dyn VblankListener>,
    latch_listener: EventListener<dyn LatchListener>,
    presentation_listener: EventListener<dyn PresentationListener>,
    commit_version: NumCell<u64>,
    latched_commit_version: Cell<u64>,
    fifo: CloneCell<Option<Rc<WpFifoV1>>>,
    clear_fifo_on_vblank: Cell<bool>,
    commit_timer: CloneCell<Option<Rc<WpCommitTimerV1>>>,
    before_latch_listener: EventListener<dyn BeforeLatchListener>,
    is_opaque: Cell<bool>,
    color_management_surface: CloneCell<Option<Rc<WpColorManagementSurfaceV1>>>,
    color_management_feedback:
        CopyHashMap<WpColorManagementSurfaceFeedbackV1Id, Rc<WpColorManagementSurfaceFeedbackV1>>,
    color_description: CloneCell<Option<Rc<ColorDescription>>>,
    tree_barriers: SyncQueue<(TreeSerial, TreeBarrier)>,
    transaction_timeline: TreeTransactionTimeline,
    unblocked_serial: Cell<TreeSerial>,
    scheduled_serial: Cell<TreeSerial>,
    acked_serial: Cell<TreeSerial>,
    tardy: Cell<bool>,
}

impl Debug for WlSurface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WlSurface").finish_non_exhaustive()
    }
}

#[derive(Default)]
struct BufferPoints {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum CommitAction {
    ContinueCommit,
    AbortCommit,
}

trait SurfaceExt {
    fn node_layer(&self) -> NodeLayerLink;

    fn commit_requested(self: Rc<Self>, pending: &mut Box<PendingState>) -> CommitAction {
        let _ = pending;
        CommitAction::ContinueCommit
    }

    fn configurable_data(&self) -> Option<&ConfigurableDataCore> {
        None
    }

    fn before_apply_commit(
        self: Rc<Self>,
        pending: &mut PendingState,
    ) -> Result<(), WlSurfaceError> {
        let _ = pending;
        Ok(())
    }

    fn after_apply_commit(self: Rc<Self>) {
        // nothing
    }

    fn is_some(&self) -> bool {
        true
    }

    fn is_none(&self) -> bool {
        !self.is_some()
    }

    fn on_surface_destroy(&self) -> Result<(), WlSurfaceError> {
        if self.is_some() {
            Err(WlSurfaceError::ReloObjectStillExists)
        } else {
            Ok(())
        }
    }

    fn subsurface_parent(&self) -> Option<Rc<WlSurface>> {
        None
    }

    fn extents_changed(&self) {
        // nothing
    }

    fn into_subsurface(self: Rc<Self>) -> Option<Rc<WlSubsurface>> {
        None
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        None
    }

    fn into_xsurface(self: Rc<Self>) -> Option<Rc<XSurface>> {
        None
    }

    fn consume_pending_child(
        &self,
        surface: &WlSurface,
        child: SubsurfaceId,
        consume: &mut dyn FnMut(
            OccupiedEntry<SubsurfaceId, AttachedSubsurfaceState>,
        ) -> Result<(), WlSurfaceError>,
    ) -> Result<(), WlSurfaceError> {
        surface.pending.borrow_mut().consume_child(child, consume)
    }

    fn tray_item(self: Rc<Self>) -> Option<TrayItemId> {
        None
    }
}

pub struct NoneSurfaceExt;

impl SurfaceExt for NoneSurfaceExt {
    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Display
    }

    fn is_some(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct PendingState {
    buffer: Option<Option<Rc<WlBuffer>>>,
    offset: (i32, i32),
    opaque_region: Option<Option<Rc<Region>>>,
    input_region: Option<Option<Rc<Region>>>,
    frame_request: Vec<Rc<WlCallback>>,
    damage_full: bool,
    buffer_damage: Vec<Rect>,
    surface_damage: Vec<Rect>,
    presentation_feedback: Vec<Rc<WpPresentationFeedback>>,
    src_rect: Option<Option<[Fixed; 4]>>,
    dst_size: Option<Option<(i32, i32)>>,
    scale: Option<i32>,
    transform: Option<Transform>,
    xwayland_serial: Option<u64>,
    tearing: Option<bool>,
    content_type: Option<Option<ContentType>>,
    xdg_surface: Option<Box<PendingXdgSurfaceData>>,
    layer_surface: Option<Box<PendingLayerSurfaceData>>,
    subsurfaces: AHashMap<SubsurfaceId, AttachedSubsurfaceState>,
    acquire_point: Option<(Rc<SyncObj>, SyncObjPoint)>,
    release_point: Option<(Rc<SyncObj>, SyncObjPoint)>,
    alpha_multiplier: Option<Option<f32>>,
    explicit_sync: bool,
    fifo_barrier_set: bool,
    fifo_barrier_wait: bool,
    commit_time: Option<u64>,
    color_description: Option<Option<Rc<ColorDescription>>>,
    serial: Option<TreeSerial>,
}

struct AttachedSubsurfaceState {
    subsurface: Rc<WlSubsurface>,
    pending: PendingSubsurfaceData,
}

impl PendingState {
    fn merge(&mut self, next: &mut Self, client: &Rc<Client>) {
        // discard state

        if next.buffer.is_some() {
            if let Some((sync_obj, point)) = self.release_point.take() {
                client.state.signal_point(&sync_obj, point);
            } else if let Some(Some(prev)) = self.buffer.take() {
                if !prev.destroyed() {
                    prev.send_release();
                }
            }
        }
        for fb in self.presentation_feedback.drain(..) {
            fb.send_discarded();
            let _ = client.remove_obj(&*fb);
        }

        // overwrite state

        if let Some(buffer) = next.buffer.take() {
            self.buffer = Some(buffer);
            self.acquire_point = next.acquire_point.take();
            self.release_point = next.release_point.take();
            self.explicit_sync = mem::take(&mut next.explicit_sync);
        }
        macro_rules! opt {
            ($name:ident) => {
                if let Some(n) = next.$name.take() {
                    self.$name = Some(n);
                }
            };
        }
        opt!(opaque_region);
        opt!(input_region);
        opt!(src_rect);
        opt!(dst_size);
        opt!(scale);
        opt!(transform);
        opt!(xwayland_serial);
        opt!(tearing);
        opt!(content_type);
        opt!(alpha_multiplier);
        opt!(commit_time);
        opt!(color_description);
        opt!(serial);
        {
            let (dx1, dy1) = self.offset;
            let (dx2, dy2) = mem::take(&mut next.offset);
            self.offset = (dx1 + dx2, dy1 + dy2);
        }
        self.frame_request.append(&mut next.frame_request);
        self.damage_full |= mem::take(&mut next.damage_full);
        if !self.damage_full {
            if self.buffer_damage.len() + next.buffer_damage.len() > MAX_DAMAGE {
                self.damage_full();
            } else {
                self.buffer_damage.append(&mut next.buffer_damage);
            }
        }
        if !self.damage_full {
            if self.surface_damage.len() + next.surface_damage.len() > MAX_DAMAGE {
                self.damage_full();
            } else {
                self.surface_damage.append(&mut next.surface_damage);
            }
        }
        next.surface_damage.clear();
        next.buffer_damage.clear();
        mem::swap(
            &mut self.presentation_feedback,
            &mut next.presentation_feedback,
        );
        self.fifo_barrier_set |= mem::take(&mut next.fifo_barrier_set);
        self.fifo_barrier_wait |= mem::take(&mut next.fifo_barrier_wait);
        macro_rules! merge_ext {
            ($name:ident) => {
                if let Some(e) = &mut self.$name {
                    if let Some(n) = &mut next.$name {
                        e.merge(n);
                    }
                } else {
                    self.$name = next.$name.take();
                }
            };
        }
        merge_ext!(xdg_surface);
        merge_ext!(layer_surface);
        for (id, mut state) in next.subsurfaces.drain() {
            match self.subsurfaces.entry(id) {
                Entry::Occupied(mut o) => {
                    o.get_mut().pending.merge(&mut state.pending, client);
                }
                Entry::Vacant(v) => {
                    v.insert(state);
                }
            }
        }
    }

    fn consume_child(
        &mut self,
        child: SubsurfaceId,
        consume: impl FnOnce(
            OccupiedEntry<SubsurfaceId, AttachedSubsurfaceState>,
        ) -> Result<(), WlSurfaceError>,
    ) -> Result<(), WlSurfaceError> {
        match self.subsurfaces.entry(child) {
            Entry::Occupied(oe) => consume(oe),
            _ => Ok(()),
        }
    }

    fn damage_full(&mut self) {
        self.damage_full = true;
        self.buffer_damage.clear();
        self.surface_damage.clear();
    }

    fn has_damage(&self) -> bool {
        self.damage_full || self.buffer_damage.is_not_empty() || self.surface_damage.is_not_empty()
    }
}

#[derive(Default)]
pub struct ParentData {
    subsurfaces: AHashMap<WlSurfaceId, Rc<WlSubsurface>>,
    pub below: LinkedList<StackElement>,
    pub above: LinkedList<StackElement>,
}

pub struct StackElement {
    pub pending: Cell<bool>,
    pub sub_surface: Rc<WlSubsurface>,
}

impl WlSurface {
    pub fn new(id: WlSurfaceId, client: &Rc<Client>, version: Version, slf: &Weak<Self>) -> Self {
        let dummy_output = client.state.dummy_output.get().unwrap();
        Self {
            id,
            node_id: client.state.node_ids.next(),
            client: client.clone(),
            visible: Cell::new(false),
            role: Cell::new(SurfaceRole::None),
            pending: Default::default(),
            input_region: Default::default(),
            opaque_region: Default::default(),
            buffer_points: Default::default(),
            buffer_points_norm: Default::default(),
            damage_matrix: Default::default(),
            buffer_transform: Cell::new(Transform::None),
            buffer_scale: Cell::new(1),
            src_rect: Cell::new(None),
            dst_size: Cell::new(None),
            extents: Default::default(),
            buffer_abs_pos: Cell::new(Default::default()),
            need_extents_update: Default::default(),
            need_extents_propagation: Default::default(),
            buffer: Default::default(),
            shm_staging: Default::default(),
            shm_textures: DoubleBuffered::new(DamageQueue::new().map(|damage| SurfaceShmTexture {
                tex: Default::default(),
                damage,
            })),
            buf_x: Default::default(),
            buf_y: Default::default(),
            children: Default::default(),
            ext: CloneCell::new(client.state.none_surface_ext.clone()),
            frame_requests: Default::default(),
            presentation_feedback: Default::default(),
            latched_presentation_feedback: Default::default(),
            seat_state: Default::default(),
            toplevel: Default::default(),
            cursors: Default::default(),
            dnd_icons: Default::default(),
            tracker: Default::default(),
            idle_inhibitors: Default::default(),
            viewporter: Default::default(),
            location: Cell::new(NodeLocation::Output(dummy_output.id)),
            output: CloneCell::new(dummy_output),
            fractional_scale: Default::default(),
            constraints: Default::default(),
            xwayland_serial: Default::default(),
            tearing_control: Default::default(),
            tearing: Cell::new(false),
            version,
            has_content_type_manager: Default::default(),
            content_type: Default::default(),
            drm_feedback: Default::default(),
            sync_obj_surface: Default::default(),
            destroyed: Cell::new(false),
            commit_timeline: client.commit_timelines.create_timeline(),
            alpha_modifier: Default::default(),
            alpha: Default::default(),
            text_input_connections: Default::default(),
            vblank_listener: EventListener::new(slf.clone()),
            latch_listener: EventListener::new(slf.clone()),
            presentation_listener: EventListener::new(slf.clone()),
            commit_version: Default::default(),
            latched_commit_version: Default::default(),
            fifo: Default::default(),
            clear_fifo_on_vblank: Default::default(),
            commit_timer: Default::default(),
            before_latch_listener: EventListener::new(slf.clone()),
            is_opaque: Cell::new(false),
            color_management_surface: Default::default(),
            color_management_feedback: Default::default(),
            color_description: Default::default(),
            tree_barriers: Default::default(),
            transaction_timeline: client.state.tree_transactions.timeline(),
            unblocked_serial: Cell::new(TreeSerial::from_raw(0)),
            scheduled_serial: Cell::new(TreeSerial::from_raw(0)),
            acked_serial: Cell::new(TreeSerial::from_raw(0)),
            tardy: Cell::new(false),
        }
    }

    fn get_xsurface(self: &Rc<Self>) -> Result<Rc<XSurface>, WlSurfaceError> {
        self.set_role(SurfaceRole::XSurface)?;
        let mut ext = self.ext.get();
        if ext.is_none() {
            let xsurface = Rc::new(XSurface {
                surface: self.clone(),
                xwindow: Default::default(),
                xwayland_surface: Default::default(),
                tracker: Default::default(),
            });
            track!(self.client, xsurface);
            self.ext.set(xsurface.clone());
            ext = xsurface;
        }
        Ok(ext.into_xsurface().unwrap())
    }

    pub fn get_output(&self) -> Rc<OutputNode> {
        self.output.get()
    }

    pub fn set_output(&self, output: &Rc<OutputNode>, location: NodeLocation) {
        self.location.set(location);
        let old = self.output.set(output.clone());
        if old.id == output.id {
            return;
        }
        if self.visible.get() {
            self.attach_events_to_output(output);
        }
        output.global.send_enter(self);
        old.global.send_leave(self);
        if old.global.persistent.scale.get() != output.global.persistent.scale.get() {
            self.on_scale_change();
        }
        if old.global.persistent.transform.get() != output.global.persistent.transform.get() {
            self.send_preferred_buffer_transform();
        }
        if old.global.color_description.get().id != output.global.color_description.get().id {
            self.send_preferred_color_description();
        }
        let children = self.children.borrow_mut();
        if let Some(children) = &*children {
            for ss in children.subsurfaces.values() {
                ss.surface.set_output(output, location);
            }
        }
    }

    fn on_scale_change(&self) {
        if let Some(fs) = self.fractional_scale.get() {
            fs.send_preferred_scale();
        }
        self.send_preferred_buffer_scale();
    }

    pub fn get_toplevel(&self) -> Option<Rc<dyn ToplevelNode>> {
        self.toplevel.get()
    }

    pub fn xwayland_serial(&self) -> Option<u64> {
        self.xwayland_serial.get()
    }

    fn set_absolute_position(&self, x1: i32, y1: i32) {
        let old_pos = self.buffer_abs_pos.get();
        let new_pos = old_pos.at_point(x1, y1);
        if self.visible.get() && self.toplevel.is_none() {
            self.client.state.damage(old_pos);
            self.client.state.damage(new_pos);
        }
        self.buffer_abs_pos.set(new_pos);
        if let Some(children) = self.children.borrow_mut().deref_mut() {
            for ss in children.subsurfaces.values() {
                let pos = ss.position.get();
                ss.surface.set_absolute_position(x1 + pos.0, y1 + pos.1);
            }
        }
        for (_, con) in &self.text_input_connections {
            for (_, popup) in &con.input_method.popups {
                popup.schedule_positioning();
            }
        }
    }

    pub fn add_presentation_feedback(&self, fb: &Rc<WpPresentationFeedback>) {
        self.pending
            .borrow_mut()
            .presentation_feedback
            .push(fb.clone());
    }

    pub fn is_cursor(&self) -> bool {
        self.role.get() == SurfaceRole::Cursor
    }

    pub fn get_cursor(
        self: &Rc<Self>,
        user: &Rc<CursorUser>,
    ) -> Result<Rc<CursorSurface>, WlSurfaceError> {
        if let Some(cursor) = self.cursors.get(&user.id) {
            return Ok(cursor);
        }
        self.set_role(SurfaceRole::Cursor)?;
        let cursor = Rc::new(CursorSurface::new(user, self));
        track!(self.client, cursor);
        cursor.handle_buffer_change();
        Ok(cursor)
    }

    pub fn get_focus_node(&self) -> Option<Rc<dyn Node>> {
        match self.toplevel.get() {
            Some(tl) if tl.tl_accepts_keyboard_focus() => tl.tl_focus_child(),
            Some(_) => None,
            _ => self.ext.get().focus_node(),
        }
    }

    pub fn send_enter(&self, output: WlOutputId) {
        self.client.event(Enter {
            self_id: self.id,
            output,
        })
    }

    pub fn send_leave(&self, output: WlOutputId) {
        self.client.event(Leave {
            self_id: self.id,
            output,
        })
    }

    pub fn send_preferred_buffer_scale(&self) {
        if self.version >= BUFFER_SCALE_SINCE {
            let factor = match self.client.wire_scale.is_some() {
                true => 1,
                false => self.output.get().global.legacy_scale.get() as _,
            };
            self.client.event(PreferredBufferScale {
                self_id: self.id,
                factor,
            });
        }
    }

    pub fn send_preferred_buffer_transform(&self) {
        if self.version >= TRANSFORM_SINCE {
            self.client.event(PreferredBufferTransform {
                self_id: self.id,
                transform: self.output.get().global.persistent.transform.get().to_wl() as _,
            });
        }
    }

    fn set_toplevel(&self, tl: Option<Rc<dyn ToplevelNode>>) {
        let ch = self.children.borrow();
        if let Some(ch) = &*ch {
            for ss in ch.subsurfaces.values() {
                ss.surface.set_toplevel(tl.clone());
            }
        }
        if self.seat_state.is_active()
            && let Some(tl) = &tl
        {
            tl.tl_surface_active_changed(true);
        }
        self.toplevel.set(tl);
    }

    pub fn set_role(&self, role: SurfaceRole) -> Result<(), WlSurfaceError> {
        use SurfaceRole::*;
        match (self.role.get(), role) {
            (None, _) => {}
            (old, new) if old == new => {}
            (old, new) => {
                return Err(WlSurfaceError::IncompatibleRole {
                    id: self.id,
                    old,
                    new,
                });
            }
        }
        self.role.set(role);
        Ok(())
    }

    pub fn into_dnd_icon(
        self: &Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
    ) -> Result<Rc<DndIcon>, WlSurfaceError> {
        self.set_role(SurfaceRole::DndIcon)?;
        Ok(Rc::new(DndIcon {
            surface: self.clone(),
            seat: seat.clone(),
        }))
    }

    fn unset_ext(&self) {
        self.ext.set(self.client.state.none_surface_ext.clone());
    }

    fn calculate_extents(&self, propagate: bool) {
        let old_extents = self.extents.get();
        let mut extents = self.buffer_abs_pos.get().at_point(0, 0);
        let children = self.children.borrow();
        if let Some(children) = &*children {
            for ss in children.subsurfaces.values() {
                let ce = ss.surface.extents.get();
                if !ce.is_empty() {
                    let cp = ss.position.get();
                    let ce = ce.move_(cp.0, cp.1);
                    extents = if extents.is_empty() {
                        ce
                    } else {
                        extents.union(ce)
                    };
                }
            }
        }
        self.extents.set(extents);
        self.need_extents_update.set(false);
        if old_extents != extents {
            if propagate {
                self.ext.get().extents_changed()
            } else {
                self.need_extents_propagation.set(true);
            }
        }
    }

    pub fn get_root(self: &Rc<Self>) -> Rc<WlSurface> {
        let mut root = self.clone();
        loop {
            if let Some(parent) = root.ext.get().subsurface_parent() {
                root = parent;
                continue;
            }
            break;
        }
        root
    }

    fn unset_cursors(&self) {
        while let Some((_, cursor)) = self.cursors.pop() {
            cursor.handle_surface_destroy();
        }
    }

    fn unset_dnd_icons(&self) {
        while let Some((_, dnd_icon)) = self.dnd_icons.pop() {
            dnd_icon.seat.remove_dnd_icon();
            if self.visible.get() {
                dnd_icon.damage();
            }
        }
    }

    fn do_damage<F>(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        f: F,
    ) -> Result<(), WlSurfaceError>
    where
        F: Fn(&mut PendingState) -> &mut Vec<Rect>,
    {
        let pending = &mut *self.pending.borrow_mut();
        if !pending.damage_full {
            let damage = f(pending);
            if damage.len() >= MAX_DAMAGE {
                pending.damage_full();
            } else {
                let Some(rect) = Rect::new_sized(x, y, width, height) else {
                    return Err(WlSurfaceError::InvalidRect);
                };
                damage.push(rect);
            }
        }
        Ok(())
    }

    pub fn handle_xwayland_wire_scale_change(&self) {
        self.send_preferred_buffer_scale();
        if let Some(fs) = self.fractional_scale.get() {
            fs.send_preferred_scale();
        }
        if let Some(xsurface) = self.ext.get().into_xsurface()
            && let Some(window) = xsurface.xwindow.get()
        {
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::Configure(window));
        }
    }

    #[cfg(feature = "it")]
    pub fn get_pending_damage(&self) -> (Vec<Rect>, Vec<Rect>, bool) {
        let pending = self.pending.borrow();
        (
            pending.surface_damage.clone(),
            pending.buffer_damage.clone(),
            pending.damage_full,
        )
    }
}

const MAX_DAMAGE: usize = 32;

impl WlSurfaceRequestHandler for WlSurface {
    type Error = WlSurfaceError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.commit_timeline.clear(ClearReason::Destroy);
        self.unset_dnd_icons();
        self.unset_cursors();
        self.ext.get().on_surface_destroy()?;
        self.destroy_node();
        {
            let mut children = self.children.borrow_mut();
            if let Some(children) = &mut *children {
                for ss in children.subsurfaces.values() {
                    ss.surface.unset_ext();
                }
            }
            *children = None;
        }
        self.buffer.set(None);
        self.reset_shm_textures();
        if let Some(xwayland_serial) = self.xwayland_serial.get() {
            self.client
                .surfaces_by_xwayland_serial
                .remove(&xwayland_serial);
        }
        self.frame_requests.borrow_mut().clear();
        self.toplevel.set(None);
        self.client.remove_obj(self)?;
        self.idle_inhibitors.clear();
        self.constraints.take();
        self.destroyed.set(true);
        Ok(())
    }

    fn attach(&self, req: Attach, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pending = &mut *self.pending.borrow_mut();
        if self.version >= OFFSET_SINCE {
            if req.x != 0 || req.y != 0 {
                return Err(WlSurfaceError::OffsetInAttach);
            }
        } else {
            pending.offset = (req.x, req.y);
        }
        let buf = if req.buffer.is_some() {
            Some(self.client.lookup(req.buffer)?)
        } else {
            None
        };
        pending.buffer = Some(buf);
        Ok(())
    }

    fn damage(&self, req: Damage, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.do_damage(req.x, req.y, req.width, req.height, |p| {
            &mut p.surface_damage
        })
    }

    fn frame(&self, req: Frame, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let cb = Rc::new(WlCallback::new(req.callback, &self.client));
        track!(self.client, cb);
        self.client.add_client_obj(&cb)?;
        self.pending.borrow_mut().frame_request.push(cb);
        Ok(())
    }

    fn set_opaque_region(
        &self,
        region: SetOpaqueRegion,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let region = if region.region.is_some() {
            Some(self.client.lookup(region.region)?.region())
        } else {
            None
        };
        self.pending.borrow_mut().opaque_region = Some(region);
        Ok(())
    }

    fn set_input_region(&self, req: SetInputRegion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let region = if req.region.is_some() {
            Some(self.client.lookup(req.region)?.region())
        } else {
            None
        };
        self.pending.borrow_mut().input_region = Some(region);
        Ok(())
    }

    fn commit(&self, _req: Commit, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let ext = self.ext.get();
        let pending = &mut *self.pending.borrow_mut();
        self.verify_explicit_sync(pending)?;
        if ext.commit_requested(pending) == CommitAction::ContinueCommit {
            self.commit_timeline.commit(slf, pending)?;
        }
        Ok(())
    }

    fn set_buffer_transform(
        &self,
        req: SetBufferTransform,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let Some(tf) = Transform::from_wl(req.transform) else {
            return Err(WlSurfaceError::UnknownBufferTransform(req.transform));
        };
        self.pending.borrow_mut().transform = Some(tf);
        Ok(())
    }

    fn set_buffer_scale(&self, req: SetBufferScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.scale < 1 {
            return Err(WlSurfaceError::NonPositiveBufferScale);
        }
        self.pending.borrow_mut().scale = Some(req.scale);
        Ok(())
    }

    fn damage_buffer(&self, req: DamageBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.do_damage(req.x, req.y, req.width, req.height, |p| {
            &mut p.buffer_damage
        })
    }

    fn offset(&self, req: Offset, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending.borrow_mut().offset = (req.x, req.y);
        Ok(())
    }
}

impl WlSurface {
    fn apply_state(self: &Rc<Self>, pending: &mut PendingState) -> Result<(), WlSurfaceError> {
        for (_, pending) in &mut pending.subsurfaces {
            pending.subsurface.apply_state(&mut pending.pending)?;
        }
        if self.destroyed.get() {
            return Ok(());
        }
        let was_visible = self.visible.get();
        self.ext.get().before_apply_commit(pending)?;
        let mut scale_changed = false;
        if let Some(scale) = pending.scale.take() {
            scale_changed = true;
            self.buffer_scale.set(scale);
        }
        let mut buffer_transform_changed = false;
        if let Some(transform) = pending.transform.take() {
            buffer_transform_changed = true;
            self.buffer_transform.set(transform);
        }
        let mut viewport_changed = false;
        if let Some(dst_size) = pending.dst_size.take() {
            viewport_changed = true;
            self.dst_size.set(dst_size);
        }
        if let Some(src_rect) = pending.src_rect.take() {
            viewport_changed = true;
            self.src_rect.set(src_rect);
        }
        if viewport_changed
            && let Some(rect) = self.src_rect.get()
            && self.dst_size.is_none()
            && (!rect[2].is_integer() || !rect[3].is_integer())
        {
            return Err(WlSurfaceError::NonIntegerViewportSize);
        }
        let mut color_description_changed = false;
        if let Some(desc) = pending.color_description.take() {
            color_description_changed = true;
            self.color_description.set(desc);
        }
        let mut alpha_changed = false;
        if let Some(alpha) = pending.alpha_multiplier.take() {
            alpha_changed = true;
            self.alpha.set(alpha);
        }
        let buffer_abs_pos = self.buffer_abs_pos.get();
        let mut max_surface_size = buffer_abs_pos.size();
        let mut damage_full = scale_changed
            || buffer_transform_changed
            || viewport_changed
            || alpha_changed
            || color_description_changed;
        let mut buffer_changed = false;
        let mut old_raw_size = None;
        let (mut dx, mut dy) = mem::take(&mut pending.offset);
        if let Some(buffer_change) = pending.buffer.take() {
            buffer_changed = true;
            if let Some(buffer) = self.buffer.take() {
                old_raw_size = Some(buffer.buffer.rect);
            }
            if let Some(buffer) = buffer_change {
                if buffer.is_shm() {
                    self.shm_textures.flip();
                    self.shm_textures.front().damage.clear();
                } else {
                    self.reset_shm_textures();
                }
                buffer.update_texture_or_log(self, false);
                let release_sync = match pending.explicit_sync {
                    false => ReleaseSync::Implicit,
                    true => ReleaseSync::Explicit,
                };
                let release = pending
                    .release_point
                    .take()
                    .map(|(sync_obj, point)| SurfaceBufferExplicitRelease { sync_obj, point });
                let surface_buffer = SurfaceBuffer {
                    buffer,
                    sync_files: Default::default(),
                    release_sync,
                    release,
                };
                self.buffer.set(Some(Rc::new(surface_buffer)));
            } else {
                self.reset_shm_textures();
                self.buf_x.set(0);
                self.buf_y.set(0);
                for (_, cursor) in &self.cursors {
                    cursor.set_hotspot(0, 0);
                }
            }
        }
        if self.buffer.is_some() && (dx, dy) != (0, 0) {
            // This is somewhat problematic since we don't accumulate small changes.
            client_wire_scale_to_logical!(self.client, dx, dy);
            self.buf_x.fetch_add(dx);
            self.buf_y.fetch_add(dy);
            self.need_extents_update.set(true);
            for (_, cursor) in &self.cursors {
                cursor.dec_hotspot(dx, dy);
            }
        }
        let transform_changed = viewport_changed || scale_changed || buffer_transform_changed;
        let mut buffer_abs_pos_size_changed = false;
        if buffer_changed || transform_changed {
            let mut buffer_points = self.buffer_points.borrow_mut();
            let mut buffer_points_norm = self.buffer_points_norm.borrow_mut();
            let mut new_size = None;
            if let Some(src_rect) = self.src_rect.get() {
                if transform_changed {
                    let [mut x1, mut y1, mut width, mut height] = src_rect.map(|v| v.to_f64() as _);
                    let scale = self.buffer_scale.get();
                    if scale != 1 {
                        let scale = scale as f32;
                        x1 *= scale;
                        y1 *= scale;
                        width *= scale;
                        height *= scale;
                    }
                    *buffer_points = BufferPoints {
                        x1,
                        y1,
                        x2: x1 + width,
                        y2: y1 + height,
                    };
                }
                let size = match self.dst_size.get() {
                    Some(ds) => ds,
                    None => (src_rect[2].to_int(), src_rect[3].to_int()),
                };
                new_size = Some(size);
            } else if let Some(size) = self.dst_size.get() {
                new_size = Some(size);
            }
            if let Some(buffer) = self.buffer.get() {
                if new_size.is_none() {
                    let (mut width, mut height) = self
                        .buffer_transform
                        .get()
                        .maybe_swap(buffer.buffer.rect.size());
                    let scale = self.buffer_scale.get();
                    if scale != 1 {
                        width = (width + scale - 1) / scale;
                        height = (height + scale - 1) / scale;
                    }
                    new_size = Some((width, height));
                }
                if transform_changed || Some(buffer.buffer.rect) != old_raw_size {
                    let (x1, y1, x2, y2) = if self.src_rect.is_none() {
                        (0.0, 0.0, 1.0, 1.0)
                    } else {
                        let (width, height) = self
                            .buffer_transform
                            .get()
                            .maybe_swap(buffer.buffer.rect.size());
                        let width = width as f32;
                        let height = height as f32;
                        let x1 = buffer_points.x1 / width;
                        let x2 = buffer_points.x2 / width;
                        let y1 = buffer_points.y1 / height;
                        let y2 = buffer_points.y2 / height;
                        if x1 > 1.0 || x2 > 1.0 || y1 > 1.0 || y2 > 1.0 {
                            return Err(WlSurfaceError::ViewportOutsideBuffer);
                        }
                        (x1, y1, x2, y2)
                    };
                    *buffer_points_norm = SampleRect {
                        x1,
                        y1,
                        x2,
                        y2,
                        buffer_transform: self.buffer_transform.get(),
                    };
                    let (buffer_width, buffer_height) = buffer.buffer.rect.size();
                    let (mut dst_width, mut dst_height) = new_size.unwrap_or_default();
                    client_wire_scale_to_logical!(self.client, dst_width, dst_height);
                    let damage_matrix = DamageMatrix::new(
                        self.buffer_transform.get(),
                        self.buffer_scale.get(),
                        buffer_width,
                        buffer_height,
                        self.src_rect.get(),
                        dst_width,
                        dst_height,
                    );
                    self.damage_matrix.set(damage_matrix);
                }
            }
            let (mut width, mut height) = new_size.unwrap_or_default();
            client_wire_scale_to_logical!(self.client, width, height);
            let (old_width, old_height) = buffer_abs_pos.size();
            if (width, height) != (old_width, old_height) {
                self.need_extents_update.set(true);
                self.buffer_abs_pos
                    .set(buffer_abs_pos.with_size(width, height).unwrap());
                max_surface_size = (width.max(old_width), height.max(old_height));
                damage_full = true;
                buffer_abs_pos_size_changed = true;
            }
        }
        let has_new_frame_requests = pending.frame_request.is_not_empty();
        {
            let frs = &mut *self.frame_requests.borrow_mut();
            frs.append(&mut pending.frame_request);
        }
        let has_presentation_feedback = {
            let mut fbs = self.presentation_feedback.borrow_mut();
            for fb in fbs.drain(..) {
                fb.send_discarded();
                let _ = self.client.remove_obj(&*fb);
            }
            mem::swap(fbs.deref_mut(), &mut pending.presentation_feedback);
            fbs.is_not_empty()
        };
        let mut opaque_region_changed = false;
        {
            if let Some(region) = pending.input_region.take() {
                self.input_region.set(region);
                self.client.state.tree_changed();
            }
            if let Some(region) = pending.opaque_region.take() {
                opaque_region_changed = true;
                self.opaque_region.set(region);
            }
        }
        if opaque_region_changed || buffer_abs_pos_size_changed {
            let pos = self.buffer_abs_pos.get().at_point(0, 0);
            let is_opaque = match self.opaque_region.get() {
                None => false,
                Some(o) => o.contains_rect(&pos),
            };
            self.is_opaque.set(is_opaque);
        }
        let mut tearing_changed = false;
        if let Some(tearing) = pending.tearing.take()
            && self.tearing.replace(tearing) != tearing
        {
            tearing_changed = true;
        }
        if let Some(content_type) = pending.content_type.take() {
            self.content_type.set(content_type);
        }
        if let Some(xwayland_serial) = pending.xwayland_serial.take() {
            self.xwayland_serial.set(Some(xwayland_serial));
            self.client
                .surfaces_by_xwayland_serial
                .set(xwayland_serial, self.clone());
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::SurfaceSerialAssigned(self.id));
        }
        if self.need_extents_update.get() {
            self.calculate_extents(false);
        }
        if buffer_changed || transform_changed || alpha_changed {
            for (_, cursor) in &self.cursors {
                cursor.handle_buffer_change();
                cursor.update_hardware_cursor();
            }
        }
        self.ext.get().after_apply_commit();
        let fifo_barrier_set = mem::take(&mut pending.fifo_barrier_set);
        if fifo_barrier_set {
            self.commit_timeline.set_fifo_barrier();
        }
        if damage_full && (self.visible.get() || was_visible) {
            let mut damage = buffer_abs_pos
                .with_size(max_surface_size.0, max_surface_size.1)
                .unwrap();
            if let Some(tl) = self.toplevel.get() {
                damage = damage.intersect(tl.node_mapped_position());
            }
            self.client.state.damage(damage);
        }
        if self.visible.get() {
            let output = self.output.get();
            if has_new_frame_requests {
                self.vblank_listener.attach(&output.vblank_event);
            }
            if has_presentation_feedback || fifo_barrier_set {
                self.latch_listener.attach(&output.latch_event);
            }
            if fifo_barrier_set {
                // If we have a fifo barrier, must trigger latching.
                output.global.connector.damage();
            }
            if damage_full {
                // handled above
            } else if pending.has_damage() {
                self.apply_damage(pending);
                if has_new_frame_requests {
                    output.global.connector.damage();
                }
            } else if has_new_frame_requests && output.schedule.vrr_enabled() {
                // Frame requests must be dispatched at the highest possible frame rate.
                // Therefore we must trigger a vsync of the output as soon as possible.
                let rect = output.global.pos.get();
                self.client.state.damage(rect);
            }
        } else {
            if fifo_barrier_set {
                self.latch_listener
                    .attach(&self.client.state.const_40hz_latch);
            }
        }
        pending.buffer_damage.clear();
        pending.surface_damage.clear();
        pending.damage_full = false;
        pending.fifo_barrier_wait = false;
        if tearing_changed
            && let Some(tl) = self.toplevel.get()
            && tl.tl_data().is_fullscreen.get()
        {
            let tt = &self.client.state.tree_transaction();
            self.output.get().update_presentation_type(tt);
        }
        if self.need_extents_propagation.take() {
            self.ext.get().extents_changed();
        }
        self.commit_version.fetch_add(1);
        Ok(())
    }

    pub fn reset_shm_textures(&self) {
        self.shm_staging.take();
        for tex in &*self.shm_textures {
            tex.tex.take();
            tex.damage.clear();
        }
    }

    fn apply_damage(&self, pending: &PendingState) {
        let bounds = self.toplevel.get().and_then(|tl| tl.tl_render_bounds());
        let pos = self.buffer_abs_pos.get();
        let apply_damage = |pos: Rect| {
            if pending.damage_full {
                let mut damage = pos;
                if let Some(bounds) = bounds {
                    damage = damage.intersect(bounds);
                }
                self.client.state.damage(damage);
            } else {
                let matrix = self.damage_matrix.get();
                if let Some(buffer) = self.buffer.get() {
                    for damage in &pending.buffer_damage {
                        let mut damage =
                            matrix.apply(pos.x1(), pos.y1(), damage.intersect(buffer.buffer.rect));
                        if let Some(bounds) = bounds {
                            damage = damage.intersect(bounds);
                        }
                        self.client.state.damage(damage);
                    }
                }
                for damage in &pending.surface_damage {
                    let mut damage = damage.move_(pos.x1(), pos.y1());
                    if let Some(scale) = self.client.wire_scale.get() {
                        let x1 = damage.x1() / scale;
                        let y1 = damage.y1() / scale;
                        let x2 = (damage.x2() + scale - 1) / scale;
                        let y2 = (damage.y2() + scale - 1) / scale;
                        damage = Rect::new(x1, y1, x2, y2).unwrap();
                    }
                    damage = damage.intersect(bounds.unwrap_or(pos));
                    self.client.state.damage(damage);
                }
            }
        };
        match self.role.get() {
            SurfaceRole::Cursor => {
                for (_, cursor) in &self.cursors {
                    if cursor.needs_damage_tracking() {
                        let (x, y) = cursor.surface_position();
                        apply_damage(pos.at_point(x, y));
                    }
                }
            }
            SurfaceRole::DndIcon => {
                for (_, dnd_icon) in &self.dnd_icons {
                    let (x, y) = dnd_icon.seat.pointer_cursor().position_int();
                    let (x, y) = dnd_icon.surface_position(x, y);
                    apply_damage(pos.at_point(x, y));
                }
            }
            _ => apply_damage(pos),
        }
    }

    fn verify_explicit_sync(&self, pending: &mut PendingState) -> Result<(), WlSurfaceError> {
        pending.explicit_sync = self.sync_obj_surface.is_some();
        if !pending.explicit_sync {
            return Ok(());
        }
        let have_new_buffer = match &pending.buffer {
            None => false,
            Some(b) => b.is_some(),
        };
        match (
            pending.release_point.is_some(),
            pending.acquire_point.is_some(),
            have_new_buffer,
        ) {
            (true, true, true) => Ok(()),
            (false, false, false) => Ok(()),
            (_, _, true) => Err(WlSurfaceError::MissingSyncPoints),
            (_, _, false) => Err(WlSurfaceError::UnexpectedSyncPoints),
        }
    }

    fn accepts_input_at(&self, mut x: i32, mut y: i32) -> bool {
        let rect = self.buffer_abs_pos.get().at_point(0, 0);
        if !rect.contains(x, y) {
            return false;
        }
        if let Some(ir) = self.input_region.get() {
            logical_to_client_wire_scale!(self.client, x, y);
            if !ir.contains(x, y) {
                return false;
            }
        }
        true
    }

    fn find_surface_at(self: &Rc<Self>, x: i32, y: i32) -> Option<(Rc<Self>, i32, i32)> {
        let children = self.children.borrow();
        let children = match children.deref() {
            Some(c) => c,
            _ => {
                return if self.accepts_input_at(x, y) {
                    Some((self.clone(), x, y))
                } else {
                    None
                };
            }
        };
        let ss = |c: &LinkedList<StackElement>| {
            for child in c.rev_iter() {
                if child.pending.get() {
                    continue;
                }
                let pos = child.sub_surface.position.get();
                let ext = child.sub_surface.surface.extents.get();
                let ext = ext.move_(pos.0, pos.1);
                if ext.contains(x, y) {
                    let (x, y) = ext.translate(x, y);
                    if let Some(res) = child.sub_surface.surface.find_surface_at(x, y) {
                        return Some(res);
                    }
                }
            }
            None
        };
        if let Some(res) = ss(&children.above) {
            return Some(res);
        }
        if self.accepts_input_at(x, y) {
            return Some((self.clone(), x, y));
        }
        if let Some(res) = ss(&children.below) {
            return Some(res);
        }
        None
    }

    fn find_tree_at_(self: &Rc<Self>, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        match self.find_surface_at(x, y) {
            Some((node, x, y)) => {
                tree.push(FoundNode { node, x, y });
                FindTreeResult::AcceptsInput
            }
            _ => FindTreeResult::Other,
        }
    }

    fn attach_events_to_output(&self, output: &OutputNode) {
        self.vblank_listener.attach(&output.vblank_event);
        self.latch_listener.attach(&output.latch_event);
    }

    pub fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) == visible {
            return;
        }
        if visible {
            self.attach_events_to_output(&self.output.get());
        }
        for (_, inhibitor) in &self.idle_inhibitors {
            if visible {
                inhibitor.activate();
            } else {
                inhibitor.deactivate();
            }
        }
        let children = self.children.borrow_mut();
        if let Some(children) = children.deref() {
            for child in children.subsurfaces.values() {
                if child.surface.buffer.is_some() {
                    child.surface.set_visible(visible);
                }
            }
        }
        self.seat_state.set_visible(self, visible);
    }

    pub fn detach_node(&self, set_invisible: bool) {
        for (_, constraint) in &self.constraints {
            constraint.deactivate(true);
        }
        for (_, inhibitor) in &self.idle_inhibitors {
            inhibitor.deactivate();
        }
        let children = self.children.borrow();
        if let Some(ch) = children.deref() {
            for ss in ch.subsurfaces.values() {
                ss.surface.detach_node(set_invisible);
            }
        }
        self.seat_state.destroy_node(self);
        if self.visible.get() && self.toplevel.is_none() {
            self.client.state.damage(self.buffer_abs_pos.get());
        }
        if set_invisible {
            self.visible.set(false);
        }
    }

    pub fn destroy_node(&self) {
        self.detach_node(true);
    }

    pub fn set_content_type(&self, content_type: Option<ContentType>) {
        self.pending.borrow_mut().content_type = Some(content_type);
    }

    pub fn request_activation(&self) {
        if let Some(tl) = self.toplevel.get() {
            tl.tl_data().request_attention(&*tl);
        }
    }

    pub fn send_feedback(&self, fb: &DrmFeedback) {
        for consumer in self.drm_feedback.lock().values() {
            consumer.send_feedback(fb);
        }
    }

    fn consume_pending_child(
        &self,
        child: SubsurfaceId,
        mut consume: impl FnMut(
            OccupiedEntry<SubsurfaceId, AttachedSubsurfaceState>,
        ) -> Result<(), WlSurfaceError>,
    ) -> Result<(), WlSurfaceError> {
        self.ext
            .get()
            .consume_pending_child(self, child, &mut consume)
    }

    pub fn alpha(&self) -> Option<f32> {
        self.alpha.get()
    }

    pub fn opaque(&self) -> bool {
        self.is_opaque.get()
    }

    pub fn opaque_region(&self) -> Option<Rc<Region>> {
        self.opaque_region.get()
    }

    pub fn color_description(&self) -> Rc<ColorDescription> {
        match self.color_description.get() {
            Some(cd) => cd,
            None => self.client.state.color_manager.srgb_gamma22().clone(),
        }
    }

    pub fn add_color_management_feedback(&self, fb: &Rc<WpColorManagementSurfaceFeedbackV1>) {
        self.color_management_feedback.set(fb.id, fb.clone());
    }

    pub fn remove_color_management_feedback(&self, fb: &WpColorManagementSurfaceFeedbackV1) {
        self.color_management_feedback.remove(&fb.id);
    }

    pub fn send_preferred_color_description(&self) {
        if self.color_management_feedback.is_empty() {
            return;
        }
        let cd = self.output.get().global.color_description.get();
        for fb in self.color_management_feedback.lock().values() {
            fb.send_preferred_changed(&cd);
        }
    }

    fn pop_tree_barriers(&self, mut cond: impl FnMut(TreeSerial, &TreeBarrier) -> bool) -> bool {
        let mut popped_any = false;
        while let Some(barrier) = self.tree_barriers.pop() {
            if cond(barrier.0, &barrier.1) {
                popped_any = true;
                continue;
            }
            self.tree_barriers.push_front(barrier);
            break;
        }
        popped_any
    }

    fn handle_acked_serial(&self, serial: TreeSerial) {
        let now = self.client.state.now_nsec();
        let mut non_tardy = !self.tardy.get();
        while let Some(barrier) = self.tree_barriers.pop() {
            if !non_tardy && barrier.1.timed_out(now) {
                continue;
            }
            if barrier.0 <= serial {
                non_tardy = true;
                continue;
            }
            self.tree_barriers.push_front(barrier);
            break;
        }
        self.acked_serial.set(serial);
        if non_tardy && self.tardy.replace(false) {
            if let Some(cd) = self.ext.get().configurable_data() {
                cd.disable_tardy();
            }
        }
    }

    #[expect(dead_code)]
    fn push_tree_blocker(self: &Rc<Self>, tt: &TreeTransaction, use_last_serial: bool) {
        let ss = &self.scheduled_serial;
        let serial;
        if use_last_serial {
            serial = ss.get();
            if self.serial_is_unblocked(serial) {
                return;
            }
        } else {
            serial = tt.serial();
        }
        let barrier = if self.tardy.get() {
            tt.weak_barrier()
        } else {
            tt.barrier()
        };
        self.tree_barriers.push((serial, barrier));
        tt.add_op(
            &self.transaction_timeline,
            WlSurfaceTreeOp {
                surface: self.clone(),
            },
        );
    }

    fn serial_is_unblocked(&self, tree_serial: TreeSerial) -> bool {
        tree_serial <= self.unblocked_serial.get() || tree_serial > self.scheduled_serial.get()
    }

    fn serial_is_blocked(&self, tree_serial: TreeSerial) -> bool {
        !self.serial_is_unblocked(tree_serial)
    }
}

object_base! {
    self = WlSurface;
    version = self.version;
}

impl Object for WlSurface {
    fn break_loops(self: Rc<Self>) {
        self.unset_dnd_icons();
        self.unset_cursors();
        self.destroy_node();
        *self.children.borrow_mut() = None;
        self.unset_ext();
        mem::take(self.frame_requests.borrow_mut().deref_mut());
        self.buffer.set(None);
        self.toplevel.set(None);
        self.idle_inhibitors.clear();
        mem::take(self.pending.borrow_mut().deref_mut());
        self.presentation_feedback.borrow_mut().clear();
        self.latched_presentation_feedback.borrow_mut().clear();
        self.viewporter.take();
        self.fractional_scale.take();
        self.tearing_control.take();
        self.constraints.clear();
        self.drm_feedback.clear();
        self.commit_timeline.clear(ClearReason::BreakLoops);
        self.alpha_modifier.take();
        self.text_input_connections.clear();
        self.fifo.take();
        self.commit_timer.take();
        self.color_management_surface.take();
        self.color_management_feedback.clear();
    }
}

dedicated_add_obj!(WlSurface, WlSurfaceId, surfaces);

tree_id!(SurfaceNodeId);
impl Node for WlSurface {
    fn node_id(&self) -> NodeId {
        self.node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let children = self.children.borrow_mut();
        if let Some(c) = children.deref() {
            for child in c.subsurfaces.values() {
                visitor.visit_surface(&child.surface);
            }
        }
    }

    fn node_visible(&self) -> bool {
        self.visible.get()
    }

    fn node_mapped_position(&self) -> Rect {
        self.buffer_abs_pos.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        Some(self.output.get())
    }

    fn node_location(&self) -> Option<NodeLocation> {
        Some(self.location.get())
    }

    fn node_layer(&self) -> NodeLayerLink {
        self.ext.get().node_layer()
    }

    fn node_active_changed(&self, active: bool) {
        if let Some(tl) = self.toplevel.get() {
            tl.tl_surface_active_changed(active);
        }
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, bounds: Option<&Rect>) {
        renderer.render_surface(self, x, y, bounds);
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.client.clone())
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        self.toplevel.get()
    }

    fn node_tray_item(&self) -> Option<TrayItemId> {
        self.ext.get().tray_item()
    }

    fn node_make_visible(self: Rc<Self>, tt: &TreeTransaction) {
        if let Some(tl) = self.toplevel.get() {
            tl.node_make_visible(tt);
        }
    }

    fn node_on_key(
        &self,
        seat: &WlSeatGlobal,
        time_usec: u64,
        key: u32,
        state: KeyState,
        kb_state: &KeyboardState,
    ) {
        seat.key_surface(self, time_usec, key, state, kb_state);
    }

    fn node_on_mods(&self, seat: &WlSeatGlobal, kb_state: &KeyboardState) {
        seat.mods_surface(self, kb_state);
    }

    fn node_on_touch_down(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        seat.touch_down_surface(&self, time_usec, id, x, y)
    }

    fn node_on_touch_up(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32) {
        seat.touch_up_surface(&self, time_usec, id)
    }

    fn node_on_touch_motion(
        self: Rc<Self>,
        seat: &WlSeatGlobal,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        seat.touch_motion_surface(&self, time_usec, id, x, y)
    }

    fn node_on_touch_frame(&self, seat: &WlSeatGlobal) {
        seat.touch_frame_surface(&self)
    }

    fn node_on_touch_cancel(&self, seat: &WlSeatGlobal) {
        seat.touch_cancel_surface(&self)
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        button: u32,
        state: KeyState,
        serial: u64,
    ) {
        seat.button_surface(&self, time_usec, button, state, serial);
    }

    fn node_on_axis_event(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, event: &PendingScroll) {
        seat.scroll_surface(&self, event);
    }

    fn node_on_focus(self: Rc<Self>, seat: &WlSeatGlobal) {
        if let Some(xsurface) = self.ext.get().into_xsurface()
            && let Some(window) = xsurface.xwindow.get()
        {
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::Activate(window.data.clone()));
        }
        seat.focus_surface(&self);
    }

    fn node_on_unfocus(&self, seat: &WlSeatGlobal) {
        seat.unfocus_surface(self);
    }

    fn node_on_leave(&self, seat: &WlSeatGlobal) {
        seat.leave_surface(self);
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        seat.enter_surface(&self, x, y)
    }

    fn node_on_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        seat.motion_surface(&self, x, y)
    }

    fn node_on_pointer_relative_motion(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        seat.relative_motion_surface(self, time_usec, dx, dy, dx_unaccelerated, dy_unaccelerated);
    }

    fn node_on_dnd_drop(&self, dnd: &Dnd) {
        dnd.seat.dnd_surface_drop(self, dnd);
    }

    fn node_on_dnd_leave(&self, dnd: &Dnd) {
        dnd.seat.dnd_surface_leave(self, dnd);
    }

    fn node_on_dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed, serial: u64) {
        dnd.seat.dnd_surface_enter(self, dnd, x, y, serial);
    }

    fn node_on_dnd_motion(&self, dnd: &Dnd, time_usec: u64, x: Fixed, y: Fixed) {
        dnd.seat.dnd_surface_motion(self, dnd, time_usec, x, y);
    }

    fn node_on_swipe_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        seat.swipe_begin_surface(self, time_usec, finger_count)
    }

    fn node_on_swipe_update(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, dx: Fixed, dy: Fixed) {
        seat.swipe_update_surface(self, time_usec, dx, dy)
    }

    fn node_on_swipe_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        seat.swipe_end_surface(self, time_usec, cancelled)
    }

    fn node_on_pinch_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        seat.pinch_begin_surface(self, time_usec, finger_count)
    }

    fn node_on_pinch_update(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        seat.pinch_update_surface(self, time_usec, dx, dy, scale, rotation)
    }

    fn node_on_pinch_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        seat.pinch_end_surface(self, time_usec, cancelled)
    }

    fn node_on_hold_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        seat.hold_begin_surface(self, time_usec, finger_count)
    }

    fn node_on_hold_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        seat.hold_end_surface(self, time_usec, cancelled)
    }

    fn node_on_tablet_pad_enter(&self, pad: &Rc<TabletPad>) {
        pad.surface_enter(self);
    }

    fn node_on_tablet_pad_leave(&self, pad: &Rc<TabletPad>) {
        pad.surface_leave(self);
    }

    fn node_on_tablet_pad_button(
        &self,
        pad: &Rc<TabletPad>,
        time_usec: u64,
        button: u32,
        state: PadButtonState,
    ) {
        pad.surface_button(self, time_usec, button, state);
    }

    fn node_on_tablet_pad_mode_switch(
        &self,
        pad: &Rc<TabletPad>,
        group: &Rc<TabletPadGroup>,
        time_usec: u64,
        mode: u32,
    ) {
        pad.surface_mode_switch(self, group, time_usec, mode);
    }

    fn node_on_tablet_pad_ring(
        &self,
        pad: &Rc<TabletPad>,
        ring: &Rc<TabletPadRing>,
        source: Option<TabletRingEventSource>,
        angle: Option<f64>,
        time_usec: u64,
    ) {
        pad.surface_ring(self, ring, source, angle, time_usec);
    }

    fn node_on_tablet_pad_strip(
        &self,
        pad: &Rc<TabletPad>,
        strip: &Rc<TabletPadStrip>,
        source: Option<TabletStripEventSource>,
        position: Option<f64>,
        time_usec: u64,
    ) {
        pad.surface_strip(self, strip, source, position, time_usec);
    }

    fn node_on_tablet_pad_dial(
        &self,
        pad: &Rc<TabletPad>,
        dial: &Rc<TabletPadDial>,
        value120: i32,
        time_usec: u64,
    ) {
        pad.surface_dial(self, dial, value120, time_usec);
    }

    fn node_on_tablet_tool_leave(&self, tool: &Rc<TabletTool>, time_usec: u64) {
        tool.surface_leave(self, time_usec);
    }

    fn node_on_tablet_tool_enter(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        x: Fixed,
        y: Fixed,
    ) {
        tool.surface_enter(&self, time_usec, x, y);
    }

    fn node_on_tablet_tool_button(
        &self,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        button: u32,
        state: ToolButtonState,
    ) {
        tool.surface_button(self, time_usec, button, state);
    }

    fn node_on_tablet_tool_apply_changes(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        changes: Option<&TabletToolChanges>,
        x: Fixed,
        y: Fixed,
    ) {
        tool.surface_apply_changes(&self, time_usec, changes, x, y);
    }

    fn node_into_surface(self: Rc<Self>) -> Option<Rc<WlSurface>> {
        Some(self.clone())
    }

    fn node_is_xwayland_surface(&self) -> bool {
        self.client.is_xwayland
    }
}

#[derive(Debug, Error)]
pub enum WlSurfaceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ZwlrLayerSurfaceV1Error(Box<ZwlrLayerSurfaceV1Error>),
    #[error(transparent)]
    XdgSurfaceError(Box<XdgSurfaceError>),
    #[error("Surface {} cannot be assigned the role {} because it already has the role {}", .id, .new.name(), .old.name())]
    IncompatibleRole {
        id: WlSurfaceId,
        old: SurfaceRole,
        new: SurfaceRole,
    },
    #[error("Cannot destroy a `wl_surface` before its role object")]
    ReloObjectStillExists,
    #[error("Buffer scale is not positive")]
    NonPositiveBufferScale,
    #[error("Unknown buffer transform {0}")]
    UnknownBufferTransform(i32),
    #[error("Viewport source is not integer-sized and destination size is not set")]
    NonIntegerViewportSize,
    #[error("Viewport source is not contained in the attached buffer")]
    ViewportOutsideBuffer,
    #[error("attach request must not contain offset")]
    OffsetInAttach,
    #[error(transparent)]
    CommitTimelineError(Box<CommitTimelineError>),
    #[error("Explicit sync buffer is attached but acquire or release points are not set")]
    MissingSyncPoints,
    #[error("No buffer is attached but acquire or release point is set")]
    UnexpectedSyncPoints,
    #[error("The supplied region is invalid")]
    InvalidRect,
    #[error("There is no render context")]
    NoRenderContext,
    #[error("Could not create a shm texture")]
    CreateAsyncShmTexture(#[source] GfxError),
    #[error("Could not prepare upload to a shm texture")]
    PrepareAsyncUpload(#[source] GfxError),
    #[error("Could not register a commit timeout")]
    RegisterCommitTimeout(#[source] IoUringError),
}
efrom!(WlSurfaceError, ClientError);
efrom!(WlSurfaceError, XdgSurfaceError);
efrom!(WlSurfaceError, ZwlrLayerSurfaceV1Error);
efrom!(WlSurfaceError, CommitTimelineError);

impl VblankListener for WlSurface {
    fn after_vblank(self: Rc<Self>) {
        if self.visible.get() {
            let now = self.client.state.now_msec();
            for fr in self.frame_requests.borrow_mut().drain(..) {
                fr.send_done(now as _);
                let _ = fr.client.remove_obj(&*fr);
            }
        }
        if self.clear_fifo_on_vblank.take() {
            self.commit_timeline.clear_fifo_barrier();
        }
        self.vblank_listener.detach();
    }
}

impl BeforeLatchListener for WlSurface {
    fn before_latch(self: Rc<Self>, present: u64) -> BeforeLatchResult {
        self.commit_timeline.before_latch(&self, present)
    }
}

impl LatchListener for WlSurface {
    fn after_latch(self: Rc<Self>, _on: &OutputNode, tearing: bool) {
        if self.visible.get() {
            if self.latched_commit_version.get() < self.commit_version.get() {
                let latched = &mut *self.latched_presentation_feedback.borrow_mut();
                for pf in latched.drain(..) {
                    pf.send_discarded();
                    let _ = pf.client.remove_obj(&*pf);
                }
                latched.append(&mut self.presentation_feedback.borrow_mut());
                if latched.is_not_empty() {
                    self.presentation_listener
                        .attach(&self.output.get().presentation_event);
                }
                self.latched_commit_version.set(self.commit_version.get());
            }
        }
        if tearing && self.visible.get() {
            if self.commit_timeline.has_fifo_barrier() {
                self.vblank_listener.attach(&self.output.get().vblank_event);
                self.clear_fifo_on_vblank.set(true);
            }
        } else {
            self.commit_timeline.clear_fifo_barrier();
        }
        self.latch_listener.detach();
    }
}

impl PresentationListener for WlSurface {
    fn presented(
        self: Rc<Self>,
        output: &OutputNode,
        tv_sec: u64,
        tv_nsec: u32,
        refresh: u32,
        seq: u64,
        flags: u32,
        vrr: bool,
    ) {
        let bindings = output.global.bindings.borrow();
        let bindings = bindings.get(&self.client.id);
        for pf in self.latched_presentation_feedback.borrow_mut().drain(..) {
            if let Some(bindings) = bindings {
                for binding in bindings.values() {
                    pf.send_sync_output(binding);
                }
            }
            let mut refresh = refresh;
            if vrr && pf.version < VRR_REFRESH_SINCE {
                refresh = 0;
            }
            pf.send_presented(tv_sec, tv_nsec, refresh, seq, flags);
            let _ = pf.client.remove_obj(&*pf);
        }
        self.presentation_listener.detach();
    }
}

pub struct WlSurfaceTreeOp {
    surface: Rc<WlSurface>,
}

impl TreeTransactionOp for WlSurfaceTreeOp {
    fn unblocked(self, serial: TreeSerial, timeout: bool) {
        let surface = &self.surface;
        if timeout {
            let guilty = surface.acked_serial.get() < serial;
            if guilty {
                surface.tardy.set(true);
                surface.tree_barriers.clear();
                if let Some(cd) = surface.ext.get().configurable_data() {
                    cd.enable_tardy();
                }
            } else {
                surface.pop_tree_barriers(|_, barrier| barrier.is_unblocked());
            }
        } else if surface.tardy.get() {
            let now = surface.client.state.now_nsec();
            surface.pop_tree_barriers(|_, barrier| barrier.timed_out(now));
        }
        let us = &surface.unblocked_serial;
        if serial > us.get() {
            us.set(serial);
            surface.commit_timeline.tree_unblocked(surface);
        }
    }
}
