use {
    crate::{
        acceptor::Acceptor,
        async_engine::{AsyncEngine, SpawnedFuture},
        backend::{
            Backend, BackendDrmDevice, BackendEvent, Connector, ConnectorId, ConnectorIds,
            DrmDeviceId, DrmDeviceIds, HardwareCursorUpdate, InputDevice, InputDeviceGroupIds,
            InputDeviceId, InputDeviceIds, MonitorInfo,
        },
        backends::dummy::DummyBackend,
        cli::RunArgs,
        client::{Client, ClientId, Clients, SerialRange, NUM_CACHED_SERIAL_RANGES},
        clientmem::ClientMemOffset,
        compositor::LIBEI_SOCKET,
        config::ConfigProxy,
        cpu_worker::CpuWorker,
        cursor::{Cursor, ServerCursors},
        cursor_user::{CursorUserGroup, CursorUserGroupId, CursorUserGroupIds, CursorUserIds},
        damage::DamageVisualizer,
        dbus::Dbus,
        drm_feedback::{DrmFeedback, DrmFeedbackIds},
        ei::{
            ei_acceptor::EiAcceptor,
            ei_client::{EiClient, EiClients},
        },
        fixed::Fixed,
        forker::ForkerProxy,
        format::Format,
        gfx_api::{
            AcquireSync, BufferResv, GfxContext, GfxError, GfxFramebuffer, GfxTexture, ReleaseSync,
            SampleRect, SyncFile,
        },
        gfx_apis::create_gfx_context,
        globals::{Globals, GlobalsError, RemovableWaylandGlobal, WaylandGlobal},
        ifs::{
            ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
            ext_session_lock_v1::ExtSessionLockV1,
            ipc::{x_data_device::XIpcDeviceIds, DataOfferIds, DataSourceIds},
            jay_render_ctx::JayRenderCtx,
            jay_screencast::JayScreencast,
            jay_seat_events::JaySeatEvents,
            jay_workspace_watcher::JayWorkspaceWatcher,
            wl_drm::WlDrmGlobal,
            wl_output::{OutputGlobalOpt, OutputId, PersistentOutputState},
            wl_seat::{
                tablet::{TabletIds, TabletInit, TabletPadIds, TabletPadInit, TabletToolIds},
                SeatIds, WlSeatGlobal,
            },
            wl_surface::{
                wl_subsurface::SubsurfaceIds,
                zwp_idle_inhibitor_v1::{IdleInhibitorId, IdleInhibitorIds, ZwpIdleInhibitorV1},
                zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2,
                NoneSurfaceExt,
            },
            wp_drm_lease_connector_v1::WpDrmLeaseConnectorV1,
            wp_drm_lease_device_v1::WpDrmLeaseDeviceV1Global,
            wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1Global,
            zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
            zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
            zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1Global,
        },
        io_uring::IoUring,
        leaks::Tracker,
        logger::Logger,
        rect::Rect,
        renderer::Renderer,
        scale::Scale,
        security_context_acceptor::SecurityContextAcceptors,
        theme::{Color, Theme},
        time::Time,
        tree::{
            ContainerNode, ContainerSplit, Direction, DisplayNode, FloatNode, Node, NodeIds,
            NodeVisitorBase, OutputNode, PlaceholderNode, TearingMode, ToplevelNode,
            ToplevelNodeBase, VrrMode, WorkspaceNode,
        },
        utils::{
            activation_token::ActivationToken, asyncevent::AsyncEvent, bindings::Bindings,
            clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt, fdcloser::FdCloser,
            hash_map_ext::HashMapExt, linkedlist::LinkedList, numcell::NumCell, queue::AsyncQueue,
            refcounted::RefCounted, run_toplevel::RunToplevel,
        },
        video::{
            dmabuf::DmaBufIds,
            drm::{
                sync_obj::{SyncObj, SyncObjPoint},
                wait_for_sync_obj::WaitForSyncObj,
                Drm,
            },
        },
        wheel::Wheel,
        wire::{
            ExtForeignToplevelListV1Id, JayRenderCtxId, JaySeatEventsId, JayWorkspaceWatcherId,
            ZwpLinuxDmabufFeedbackV1Id,
        },
        xkbcommon::{KeyboardStateIds, XkbContext, XkbKeymap, XkbState},
        xwayland::{self, XWaylandEvent},
    },
    ahash::{AHashMap, AHashSet},
    bstr::ByteSlice,
    jay_config::{
        video::{GfxApi, Transform},
        PciId,
    },
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        num::Wrapping,
        ops::DerefMut,
        rc::Rc,
        sync::Arc,
        time::Duration,
    },
    thiserror::Error,
};

pub struct State {
    pub xkb_ctx: XkbContext,
    pub backend: CloneCell<Rc<dyn Backend>>,
    pub forker: CloneCell<Option<Rc<ForkerProxy>>>,
    pub default_keymap: Rc<XkbKeymap>,
    pub eng: Rc<AsyncEngine>,
    pub render_ctx: CloneCell<Option<Rc<dyn GfxContext>>>,
    pub drm_feedback: CloneCell<Option<Rc<DrmFeedback>>>,
    pub drm_feedback_consumers:
        CopyHashMap<(ClientId, ZwpLinuxDmabufFeedbackV1Id), Rc<ZwpLinuxDmabufFeedbackV1>>,
    pub render_ctx_version: NumCell<u32>,
    pub render_ctx_ever_initialized: Cell<bool>,
    pub cursors: CloneCell<Option<Rc<ServerCursors>>>,
    pub wheel: Rc<Wheel>,
    pub clients: Clients,
    pub globals: Globals,
    pub connector_ids: ConnectorIds,
    pub drm_dev_ids: DrmDeviceIds,
    pub seat_ids: SeatIds,
    pub idle_inhibitor_ids: IdleInhibitorIds,
    pub input_device_ids: InputDeviceIds,
    pub node_ids: NodeIds,
    pub root: Rc<DisplayNode>,
    pub workspaces: CopyHashMap<String, Rc<WorkspaceNode>>,
    pub dummy_output: CloneCell<Option<Rc<OutputNode>>>,
    pub backend_events: AsyncQueue<BackendEvent>,
    pub input_device_handlers: RefCell<AHashMap<InputDeviceId, InputDeviceData>>,
    pub seat_queue: LinkedList<Rc<WlSeatGlobal>>,
    pub slow_clients: AsyncQueue<Rc<Client>>,
    pub none_surface_ext: Rc<NoneSurfaceExt>,
    pub tree_changed_sent: Cell<bool>,
    pub config: CloneCell<Option<Rc<ConfigProxy>>>,
    pub theme: Theme,
    pub pending_container_layout: AsyncQueue<Rc<ContainerNode>>,
    pub pending_container_render_positions: AsyncQueue<Rc<ContainerNode>>,
    pub pending_container_render_title: AsyncQueue<Rc<ContainerNode>>,
    pub pending_output_render_data: AsyncQueue<Rc<OutputNode>>,
    pub pending_float_layout: AsyncQueue<Rc<FloatNode>>,
    pub pending_float_titles: AsyncQueue<Rc<FloatNode>>,
    pub pending_input_popup_positioning: AsyncQueue<Rc<ZwpInputPopupSurfaceV2>>,
    pub pending_toplevel_screencasts: AsyncQueue<Rc<JayScreencast>>,
    pub pending_screencast_reallocs_or_reconfigures: AsyncQueue<Rc<JayScreencast>>,
    pub pending_placeholder_render_textures: AsyncQueue<Rc<PlaceholderNode>>,
    pub dbus: Dbus,
    pub fdcloser: Arc<FdCloser>,
    pub logger: Option<Arc<Logger>>,
    pub connectors: CopyHashMap<ConnectorId, Rc<ConnectorData>>,
    pub outputs: CopyHashMap<ConnectorId, Rc<OutputData>>,
    pub drm_devs: CopyHashMap<DrmDeviceId, Rc<DrmDevData>>,
    pub status: CloneCell<Rc<String>>,
    pub idle: IdleState,
    pub run_args: RunArgs,
    pub xwayland: XWaylandState,
    pub acceptor: CloneCell<Option<Rc<Acceptor>>>,
    pub serial: NumCell<Wrapping<u32>>,
    pub run_toplevel: Rc<RunToplevel>,
    pub config_dir: Option<String>,
    pub config_file_id: NumCell<u64>,
    pub tracker: Tracker<Self>,
    pub data_offer_ids: DataOfferIds,
    pub data_source_ids: DataSourceIds,
    pub ring: Rc<IoUring>,
    pub lock: ScreenlockState,
    pub scales: RefCounted<Scale>,
    pub cursor_sizes: RefCounted<u32>,
    pub hardware_tick_cursor: AsyncQueue<Option<Rc<dyn Cursor>>>,
    pub testers: RefCell<AHashMap<(ClientId, JaySeatEventsId), Rc<JaySeatEvents>>>,
    pub render_ctx_watchers: CopyHashMap<(ClientId, JayRenderCtxId), Rc<JayRenderCtx>>,
    pub workspace_watchers: CopyHashMap<(ClientId, JayWorkspaceWatcherId), Rc<JayWorkspaceWatcher>>,
    pub default_workspace_capture: Cell<bool>,
    pub default_gfx_api: Cell<GfxApi>,
    pub activation_tokens: CopyHashMap<ActivationToken, ()>,
    pub toplevel_lists:
        CopyHashMap<(ClientId, ExtForeignToplevelListV1Id), Rc<ExtForeignToplevelListV1>>,
    pub dma_buf_ids: DmaBufIds,
    pub drm_feedback_ids: DrmFeedbackIds,
    pub direct_scanout_enabled: Cell<bool>,
    pub persistent_output_states: CopyHashMap<Rc<OutputId>, Rc<PersistentOutputState>>,
    pub double_click_interval_usec: Cell<u64>,
    pub double_click_distance: Cell<i32>,
    pub create_default_seat: Cell<bool>,
    pub subsurface_ids: SubsurfaceIds,
    pub wait_for_sync_obj: Rc<WaitForSyncObj>,
    pub explicit_sync_enabled: Cell<bool>,
    pub keyboard_state_ids: KeyboardStateIds,
    pub security_context_acceptors: SecurityContextAcceptors,
    pub cursor_user_group_ids: CursorUserGroupIds,
    pub cursor_user_ids: CursorUserIds,
    pub cursor_user_groups: CopyHashMap<CursorUserGroupId, Rc<CursorUserGroup>>,
    pub cursor_user_group_hardware_cursor: CloneCell<Option<Rc<CursorUserGroup>>>,
    pub input_device_group_ids: InputDeviceGroupIds,
    pub tablet_ids: TabletIds,
    pub tablet_tool_ids: TabletToolIds,
    pub tablet_pad_ids: TabletPadIds,
    pub damage_visualizer: DamageVisualizer,
    pub default_vrr_mode: Cell<&'static VrrMode>,
    pub default_vrr_cursor_hz: Cell<Option<f64>>,
    pub default_tearing_mode: Cell<&'static TearingMode>,
    pub ei_acceptor: CloneCell<Option<Rc<EiAcceptor>>>,
    pub ei_acceptor_future: CloneCell<Option<SpawnedFuture<()>>>,
    pub enable_ei_acceptor: Cell<bool>,
    pub ei_clients: EiClients,
    pub slow_ei_clients: AsyncQueue<Rc<EiClient>>,
    pub cpu_worker: Rc<CpuWorker>,
    pub ui_drag_enabled: Cell<bool>,
    pub ui_drag_threshold_squared: Cell<i32>,
}

// impl Drop for State {
//     fn drop(&mut self) {
//         log::info!("drop state");
//     }
// }

impl Debug for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State").finish_non_exhaustive()
    }
}

pub struct ScreenlockState {
    pub locked: Cell<bool>,
    pub lock: CloneCell<Option<Rc<ExtSessionLockV1>>>,
}

pub struct XWaylandState {
    pub enabled: Cell<bool>,
    pub handler: RefCell<Option<SpawnedFuture<()>>>,
    pub queue: Rc<AsyncQueue<XWaylandEvent>>,
    pub ipc_device_ids: XIpcDeviceIds,
}

pub struct IdleState {
    pub input: Cell<bool>,
    pub change: AsyncEvent,
    pub timeout: Cell<Duration>,
    pub timeout_changed: Cell<bool>,
    pub inhibitors: CopyHashMap<IdleInhibitorId, Rc<ZwpIdleInhibitorV1>>,
    pub inhibitors_changed: Cell<bool>,
    pub backend_idle: Cell<bool>,
}

impl IdleState {
    pub fn set_timeout(&self, timeout: Duration) {
        self.timeout.set(timeout);
        self.timeout_changed.set(true);
        self.change.trigger();
    }

    pub fn add_inhibitor(&self, inhibitor: &Rc<ZwpIdleInhibitorV1>) {
        self.inhibitors.set(inhibitor.inhibit_id, inhibitor.clone());
        self.inhibitors_changed.set(true);
        self.change.trigger();
    }

    pub fn remove_inhibitor(&self, inhibitor: &ZwpIdleInhibitorV1) {
        self.inhibitors.remove(&inhibitor.inhibit_id);
        self.inhibitors_changed.set(true);
        self.change.trigger();
    }
}

pub struct InputDeviceData {
    pub _handler: SpawnedFuture<()>,
    pub id: InputDeviceId,
    pub data: Rc<DeviceHandlerData>,
    pub async_event: Rc<AsyncEvent>,
}

pub struct DeviceHandlerData {
    pub seat: CloneCell<Option<Rc<WlSeatGlobal>>>,
    pub px_per_scroll_wheel: Cell<f64>,
    pub device: Rc<dyn InputDevice>,
    pub syspath: Option<String>,
    pub devnode: Option<String>,
    pub keymap: CloneCell<Option<Rc<XkbKeymap>>>,
    pub xkb_state: CloneCell<Option<Rc<RefCell<XkbState>>>>,
    pub output: CloneCell<Option<Rc<OutputGlobalOpt>>>,
    pub tablet_init: Option<Box<TabletInit>>,
    pub tablet_pad_init: Option<Box<TabletPadInit>>,
    pub is_touch: bool,
}

pub struct ConnectorData {
    pub connector: Rc<dyn Connector>,
    pub handler: Cell<Option<SpawnedFuture<()>>>,
    pub connected: Cell<bool>,
    pub name: String,
    pub drm_dev: Option<Rc<DrmDevData>>,
    pub async_event: Rc<AsyncEvent>,
    pub damaged: Cell<bool>,
}

pub struct OutputData {
    pub connector: Rc<ConnectorData>,
    pub monitor_info: MonitorInfo,
    pub node: Option<Rc<OutputNode>>,
    pub lease_connectors: Rc<Bindings<WpDrmLeaseConnectorV1>>,
}

pub struct DrmDevData {
    pub dev: Rc<dyn BackendDrmDevice>,
    pub handler: Cell<Option<SpawnedFuture<()>>>,
    pub connectors: CopyHashMap<ConnectorId, Rc<ConnectorData>>,
    pub syspath: Option<String>,
    pub devnode: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub pci_id: Option<PciId>,
    pub lease_global: Rc<WpDrmLeaseDeviceV1Global>,
}

impl ConnectorData {
    pub fn damage(&self) {
        if !self.damaged.replace(true) {
            self.connector.damage();
        }
    }
}

impl DrmDevData {
    pub fn make_render_device(&self) {
        log::info!(
            "Making {} the render device",
            self.devnode.as_deref().unwrap_or("unknown"),
        );
        self.dev.clone().make_render_device();
    }
}

struct UpdateTextTexturesVisitor;
impl NodeVisitorBase for UpdateTextTexturesVisitor {
    fn visit_container(&mut self, node: &Rc<ContainerNode>) {
        node.children
            .iter()
            .for_each(|c| c.title_tex.borrow_mut().clear());
        node.schedule_render_titles();
        node.node_visit_children(self);
    }
    fn visit_output(&mut self, node: &Rc<OutputNode>) {
        node.schedule_update_render_data();
        node.node_visit_children(self);
    }
    fn visit_float(&mut self, node: &Rc<FloatNode>) {
        node.title_textures.borrow_mut().clear();
        node.schedule_render_titles();
        node.node_visit_children(self);
    }
    fn visit_workspace(&mut self, node: &Rc<WorkspaceNode>) {
        node.title_texture.take();
        node.node_visit_children(self);
    }
    fn visit_placeholder(&mut self, node: &Rc<PlaceholderNode>) {
        node.textures.borrow_mut().clear();
        node.schedule_update_texture();
        node.node_visit_children(self);
    }
}

impl State {
    pub fn create_gfx_context(
        &self,
        drm: &Drm,
        api: Option<GfxApi>,
    ) -> Result<Rc<dyn GfxContext>, GfxError> {
        create_gfx_context(
            &self.eng,
            &self.ring,
            drm,
            api.unwrap_or(self.default_gfx_api.get()),
        )
    }

    pub fn add_output_scale(&self, scale: Scale) {
        if self.scales.add(scale) {
            self.output_scales_changed();
        }
    }

    pub fn remove_output_scale(&self, scale: Scale) {
        if self.scales.remove(&scale) {
            self.output_scales_changed();
        }
    }

    pub fn add_cursor_size(&self, size: u32) {
        if self.cursor_sizes.add(size) {
            self.cursor_sizes_changed();
        }
    }

    pub fn remove_cursor_size(&self, size: u32) {
        if self.cursor_sizes.remove(&size) {
            self.cursor_sizes_changed();
        }
    }

    fn output_scales_changed(&self) {
        UpdateTextTexturesVisitor.visit_display(&self.root);
        self.reload_cursors();
    }

    fn cursor_sizes_changed(&self) {
        self.reload_cursors();
    }

    pub fn devices_enumerated(&self) {
        if let Some(config) = self.config.get() {
            config.devices_enumerated()
        }
        if self.render_ctx.is_none() {
            for dev in self.drm_devs.lock().values() {
                if let Ok(version) = dev.dev.version() {
                    if version.name.contains_str("nvidia") {
                        continue;
                    }
                }
                dev.make_render_device();
                if self.render_ctx.is_some() {
                    break;
                }
            }
            if self.render_ctx.is_none() {
                if let Some(dev) = self.drm_devs.lock().values().next() {
                    dev.make_render_device();
                }
            }
        }
    }

    pub fn set_render_ctx(&self, ctx: Option<Rc<dyn GfxContext>>) {
        self.render_ctx.set(ctx.clone());
        self.render_ctx_version.fetch_add(1);
        self.cursors.set(None);
        self.drm_feedback.set(None);
        self.wait_for_sync_obj
            .set_ctx(ctx.as_ref().and_then(|c| c.sync_obj_ctx().cloned()));

        'handle_new_feedback: {
            if let Some(ctx) = &ctx {
                let feedback = match DrmFeedback::new(&self.drm_feedback_ids, &**ctx) {
                    Ok(fb) => fb,
                    Err(e) => {
                        log::error!("Could not create new DRM feedback: {}", ErrorFmt(e));
                        break 'handle_new_feedback;
                    }
                };
                for watcher in self.drm_feedback_consumers.lock().values() {
                    watcher.send_feedback(&feedback);
                }
                self.drm_feedback.set(Some(Rc::new(feedback)));
            }
        }

        {
            struct Walker;
            impl NodeVisitorBase for Walker {
                fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                    node.render_data.borrow_mut().titles.clear();
                    node.children
                        .iter()
                        .for_each(|c| c.title_tex.borrow_mut().clear());
                    node.node_visit_children(self);
                }
                fn visit_workspace(&mut self, node: &Rc<WorkspaceNode>) {
                    node.title_texture.take();
                    node.node_visit_children(self);
                }
                fn visit_output(&mut self, node: &Rc<OutputNode>) {
                    node.render_data.borrow_mut().titles.clear();
                    node.render_data.borrow_mut().status.take();
                    node.hardware_cursor.set(None);
                    node.node_visit_children(self);
                }
                fn visit_float(&mut self, node: &Rc<FloatNode>) {
                    node.title_textures.borrow_mut().clear();
                    node.node_visit_children(self);
                }
                fn visit_placeholder(&mut self, node: &Rc<PlaceholderNode>) {
                    node.textures.borrow_mut().clear();
                    node.node_visit_children(self);
                }
            }
            Walker.visit_display(&self.root);
            for client in self.clients.clients.borrow_mut().values() {
                let mut updated_buffers = AHashSet::new();
                for surface in client.data.objects.surfaces.lock().values() {
                    if let Some(buffer) = surface.buffer.get() {
                        updated_buffers.insert(buffer.buffer.id);
                        buffer.buffer.handle_gfx_context_change(Some(surface));
                    } else {
                        surface.reset_shm_textures();
                    }
                }
                for buffer in client.data.objects.buffers.lock().values() {
                    if !updated_buffers.contains(&buffer.id) {
                        buffer.handle_gfx_context_change(None);
                    }
                }
            }
        }

        if ctx.is_some() {
            self.reload_cursors();
            UpdateTextTexturesVisitor.visit_display(&self.root);
        }

        for cursor_user_groups in self.cursor_user_groups.lock().values() {
            cursor_user_groups.render_ctx_changed();
        }

        if let Some(ctx) = &ctx {
            if !self.render_ctx_ever_initialized.replace(true) {
                self.add_global(&Rc::new(WlDrmGlobal::new(self.globals.name())));
                self.add_global(&Rc::new(ZwpLinuxDmabufV1Global::new(self.globals.name())));
                if let Some(ctx) = ctx.sync_obj_ctx() {
                    if ctx.supports_async_wait() && self.explicit_sync_enabled.get() {
                        self.add_global(&Rc::new(WpLinuxDrmSyncobjManagerV1Global::new(
                            self.globals.name(),
                        )));
                    }
                }
                if let Some(config) = self.config.get() {
                    config.graphics_initialized();
                }
            }
        }

        for watcher in self.render_ctx_watchers.lock().values() {
            watcher.send_render_ctx(ctx.clone());
        }

        let mut scs = vec![];
        for client in self.clients.clients.borrow_mut().values() {
            for sc in client.data.objects.screencasts.lock().values() {
                scs.push(sc.clone());
            }
        }
        for sc in scs {
            sc.do_destroy();
        }
    }

    fn reload_cursors(&self) {
        if let Some(ctx) = self.render_ctx.get() {
            let cursors = match ServerCursors::load(&ctx, self) {
                Ok(c) => c.map(Rc::new),
                Err(e) => {
                    log::error!("Could not load the cursors: {}", ErrorFmt(e));
                    None
                }
            };
            self.cursors.set(cursors);
            for cursor_user_group in self.cursor_user_groups.lock().values() {
                cursor_user_group.reload_known_cursor();
            }
        }
    }

    pub fn add_global<T: WaylandGlobal>(&self, global: &Rc<T>) {
        self.globals.add_global(self, global)
    }

    pub fn remove_global<T: RemovableWaylandGlobal>(&self, global: &T) -> Result<(), GlobalsError> {
        self.globals.remove(self, global)
    }

    pub fn tree_changed(&self) {
        // log::info!("state.tree_changed\n{:?}", Backtrace::new());
        if self.tree_changed_sent.replace(true) {
            return;
        }
        let seats = self.globals.seats.lock();
        for seat in seats.values() {
            seat.trigger_tree_changed(false);
        }
    }

    pub fn map_tiled(self: &Rc<Self>, node: Rc<dyn ToplevelNode>) {
        let seat = self.seat_queue.last();
        self.do_map_tiled(seat.as_deref(), node.clone());
        if node.node_visible() {
            if let Some(seat) = seat {
                node.node_do_focus(&seat, Direction::Unspecified);
            }
        }
    }

    fn do_map_tiled(self: &Rc<Self>, seat: Option<&Rc<WlSeatGlobal>>, node: Rc<dyn ToplevelNode>) {
        let output = seat
            .map(|s| s.get_output())
            .or_else(|| self.root.outputs.lock().values().next().cloned())
            .or_else(|| self.dummy_output.get())
            .unwrap();
        let ws = output.ensure_workspace();
        self.map_tiled_on(node, &ws);
    }

    pub fn map_tiled_on(self: &Rc<Self>, node: Rc<dyn ToplevelNode>, ws: &Rc<WorkspaceNode>) {
        if let Some(c) = ws.container.get() {
            let la = c.clone().tl_last_active_child();
            let lap = la
                .tl_data()
                .parent
                .get()
                .and_then(|n| n.node_into_container());
            if let Some(lap) = lap {
                lap.add_child_after(la.tl_as_node(), node);
            } else {
                c.append_child(node);
            }
        } else {
            let container = ContainerNode::new(self, ws, node, ContainerSplit::Horizontal);
            ws.set_container(&container);
        }
    }

    pub fn map_floating(
        self: &Rc<Self>,
        node: Rc<dyn ToplevelNode>,
        mut width: i32,
        mut height: i32,
        workspace: &Rc<WorkspaceNode>,
        abs_pos: Option<(i32, i32)>,
    ) {
        width += 2 * self.theme.sizes.border_width.get();
        height += 2 * self.theme.sizes.border_width.get() + self.theme.sizes.title_height.get() + 1;
        let output = workspace.output.get();
        let output_rect = output.global.pos.get();
        let position = if let Some((mut x1, mut y1)) = abs_pos {
            if y1 <= output_rect.y1() {
                y1 = output_rect.y1() + 1;
            }
            if y1 > output_rect.y2() {
                y1 = output_rect.y2();
            }
            y1 -= self.theme.sizes.border_width.get() + self.theme.sizes.title_height.get() + 1;
            x1 -= self.theme.sizes.border_width.get();
            Rect::new_sized(x1, y1, width, height).unwrap()
        } else {
            let mut x1 = output_rect.x1();
            let mut y1 = output_rect.y1();
            if width < output_rect.width() {
                x1 += (output_rect.width() - width) / 2;
            } else {
                width = output_rect.width();
            }
            if height < output_rect.height() {
                y1 += (output_rect.height() - height) / 2;
            } else {
                height = output_rect.height();
            }
            Rect::new_sized(x1, y1, width, height).unwrap()
        };
        FloatNode::new(self, workspace, position, node.clone());
        if node.node_visible() {
            if let Some(seat) = self.seat_queue.last() {
                node.node_do_focus(&seat, Direction::Unspecified);
            }
        }
    }

    pub fn show_workspace(&self, seat: &Rc<WlSeatGlobal>, name: &str) {
        let (output, ws) = match self.workspaces.get(name) {
            Some(ws) => {
                let output = ws.output.get();
                let did_change = output.show_workspace(&ws);
                ws.clone().node_do_focus(seat, Direction::Unspecified);
                if !did_change {
                    return;
                }
                (output, ws)
            }
            _ => {
                let output = seat.get_output();
                if output.is_dummy {
                    log::warn!("Not showing workspace because seat is on dummy output");
                    return;
                }
                let ws = output.create_workspace(name);
                output.show_workspace(&ws);
                (output, ws)
            }
        };
        ws.flush_jay_workspaces();
        output.schedule_update_render_data();
        self.tree_changed();
        // let seats = self.globals.seats.lock();
        // for seat in seats.values() {
        //     seat.workspace_changed(&output);
        // }
    }

    pub fn float_map_ws(&self) -> Rc<WorkspaceNode> {
        if let Some(seat) = self.seat_queue.last() {
            let output = seat.get_output();
            if !output.is_dummy {
                return output.ensure_workspace();
            }
        }
        if let Some(output) = self.root.outputs.lock().values().next().cloned() {
            return output.ensure_workspace();
        }
        self.dummy_output.get().unwrap().ensure_workspace()
    }

    pub fn set_status(&self, status: &str) {
        let status = Rc::new(status.to_owned());
        self.status.set(status.clone());
        let outputs = self.root.outputs.lock();
        for output in outputs.values() {
            output.set_status(&status);
        }
    }

    pub fn input_occurred(&self) {
        if !self.idle.input.replace(true) {
            self.idle.change.trigger();
        }
    }

    pub fn start_xwayland(self: &Rc<Self>) {
        if !self.xwayland.enabled.get() {
            return;
        }
        let mut handler = self.xwayland.handler.borrow_mut();
        if handler.is_none() {
            *handler = Some(self.eng.spawn("xwayland", xwayland::manage(self.clone())));
        }
    }

    pub fn next_serial(&self, client: Option<&Client>) -> u32 {
        let serial = self.serial.fetch_add(Wrapping(1)).0;
        if let Some(client) = client {
            'update_range: {
                let mut serials = client.serials.borrow_mut();
                if let Some(last) = serials.back_mut() {
                    if last.hi.wrapping_add(1) == serial {
                        last.hi = serial;
                        break 'update_range;
                    }
                }
                if serials.len() >= NUM_CACHED_SERIAL_RANGES {
                    serials.pop_front();
                }
                serials.push_back(SerialRange {
                    lo: serial,
                    hi: serial,
                });
            }
        }
        serial as _
    }

    pub fn damage(&self, rect: Rect) {
        self.damage2(false, rect);
    }

    pub fn damage2(&self, cursor: bool, rect: Rect) {
        if rect.is_empty() {
            return;
        }
        self.damage_visualizer.add(rect);
        for output in self.root.outputs.lock().values() {
            if output.global.pos.get().intersects(&rect) {
                if cursor && output.schedule.defer_cursor_updates() {
                    output.schedule.software_cursor_changed();
                } else {
                    output.global.connector.damage();
                }
            }
        }
    }

    pub fn do_unlock(&self) {
        self.lock.locked.set(false);
        self.lock.lock.take();
        for output in self.root.outputs.lock().values() {
            if let Some(surface) = output.set_lock_surface(None) {
                surface.destroy_node();
            }
        }
        self.tree_changed();
        self.damage(self.root.extents.get());
    }

    pub fn clear(&self) {
        self.lock.lock.take();
        self.xwayland.handler.borrow_mut().take();
        self.clients.clear();
        if let Some(config) = self.config.set(None) {
            config.clear();
        }
        if let Some(forker) = self.forker.set(None) {
            forker.clear();
        }
        self.acceptor.set(None);
        self.backend.set(Rc::new(DummyBackend)).clear();
        self.run_toplevel.clear();
        self.xwayland.handler.borrow_mut().take();
        self.xwayland.queue.clear();
        self.idle.inhibitors.clear();
        self.idle.change.clear();
        for drm_dev in self.drm_devs.lock().drain_values() {
            drm_dev.handler.take();
            drm_dev.connectors.clear();
        }
        for connector in self.connectors.lock().drain_values() {
            connector.handler.take();
            connector.async_event.clear();
        }
        self.outputs.clear();
        for output in self.root.outputs.lock().values() {
            output.clear();
        }
        self.dbus.clear();
        self.pending_container_layout.clear();
        self.pending_container_render_positions.clear();
        self.pending_container_render_title.clear();
        self.pending_output_render_data.clear();
        self.pending_float_layout.clear();
        self.pending_float_titles.clear();
        self.pending_input_popup_positioning.clear();
        self.pending_toplevel_screencasts.clear();
        self.pending_screencast_reallocs_or_reconfigures.clear();
        self.pending_placeholder_render_textures.clear();
        self.render_ctx_watchers.clear();
        self.workspace_watchers.clear();
        self.toplevel_lists.clear();
        self.security_context_acceptors.clear();
        self.slow_clients.clear();
        for h in self.input_device_handlers.borrow_mut().drain_values() {
            h.async_event.clear();
        }
        self.backend_events.clear();
        self.workspaces.clear();
        {
            let seats = mem::take(self.globals.seats.lock().deref_mut());
            for seat in seats.values() {
                seat.clear();
            }
        }
        self.globals.clear();
        self.render_ctx.set(None);
        self.root.clear();
        if let Some(output) = self.dummy_output.set(None) {
            output.clear();
        }
        self.wheel.clear();
        self.eng.clear();
        self.ei_acceptor.take();
        self.ei_acceptor_future.take();
        self.ei_clients.clear();
        self.slow_ei_clients.clear();
    }

    pub fn damage_hardware_cursors(&self, render: bool) {
        for output in self.root.outputs.lock().values() {
            if let Some(hc) = output.hardware_cursor.get() {
                if render {
                    output.hardware_cursor_needs_render.set(true);
                }
                hc.damage();
            }
        }
    }

    pub fn refresh_hardware_cursors(&self) {
        if let Some(g) = self.cursor_user_group_hardware_cursor.get() {
            if let Some(u) = g.active() {
                u.update_hardware_cursor();
                return;
            }
        }
        self.damage_hardware_cursors(false)
    }

    pub fn present_hardware_cursor(
        &self,
        output: &Rc<OutputNode>,
        hc: &mut dyn HardwareCursorUpdate,
    ) {
        let Some(g) = self.cursor_user_group_hardware_cursor.get() else {
            hc.set_enabled(false);
            return;
        };
        g.present_hardware_cursor(output, hc);
    }

    pub fn for_each_seat_tester<F: Fn(&JaySeatEvents)>(&self, f: F) {
        let testers = self.testers.borrow_mut();
        for tester in testers.values() {
            f(tester);
        }
    }

    pub fn present_output(
        &self,
        output: &OutputNode,
        fb: &Rc<dyn GfxFramebuffer>,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        tex: &Rc<dyn GfxTexture>,
        render_hw_cursor: bool,
    ) -> Result<Option<SyncFile>, GfxError> {
        let sync_file = fb.render_output(
            acquire_sync,
            release_sync,
            output,
            self,
            Some(output.global.pos.get()),
            output.global.persistent.scale.get(),
            render_hw_cursor,
        )?;
        output.latched();
        output.perform_screencopies(
            tex,
            None,
            &AcquireSync::Unnecessary,
            ReleaseSync::None,
            !render_hw_cursor,
            0,
            0,
            None,
        );
        Ok(sync_file)
    }

    pub fn perform_screencopy(
        &self,
        src: &Rc<dyn GfxTexture>,
        resv: Option<&Rc<dyn BufferResv>>,
        acquire_sync: &AcquireSync,
        release_sync: ReleaseSync,
        target: &Rc<dyn GfxFramebuffer>,
        target_acquire_sync: AcquireSync,
        target_release_sync: ReleaseSync,
        target_transform: Transform,
        position: Rect,
        render_hardware_cursors: bool,
        x_off: i32,
        y_off: i32,
        size: Option<(i32, i32)>,
        transform: Transform,
        scale: Scale,
    ) -> Result<Option<SyncFile>, GfxError> {
        let mut ops = vec![];
        let mut renderer = Renderer {
            base: target.renderer_base(&mut ops, scale, target_transform),
            state: self,
            logical_extents: position.at_point(0, 0),
            pixel_extents: {
                let (width, height) = target.logical_size(target_transform);
                Rect::new_sized(0, 0, width, height).unwrap()
            },
        };
        let mut sample_rect = SampleRect::identity();
        sample_rect.buffer_transform = transform;
        renderer.base.render_texture(
            src,
            None,
            x_off,
            y_off,
            Some(sample_rect),
            size,
            scale,
            None,
            resv.cloned(),
            acquire_sync.clone(),
            release_sync,
        );
        if render_hardware_cursors {
            if let Some(cursor_user_group) = self.cursor_user_group_hardware_cursor.get() {
                if let Some(cursor_user) = cursor_user_group.active() {
                    if let Some(cursor) = cursor_user.get() {
                        let (mut x, mut y) = cursor_user.position();
                        x = x + x_off - Fixed::from_int(position.x1());
                        y = y + y_off - Fixed::from_int(position.y1());
                        cursor.render(&mut renderer, x, y);
                    }
                }
            }
        }
        target.render(
            target_acquire_sync,
            target_release_sync,
            &ops,
            Some(&Color::SOLID_BLACK),
        )
    }

    fn have_hardware_cursor(&self) -> bool {
        if let Some(group) = self.cursor_user_group_hardware_cursor.get() {
            if let Some(user) = group.active() {
                if user.get().is_some() {
                    return true;
                }
            }
        }
        false
    }

    pub fn perform_shm_screencopy(
        &self,
        src: &Rc<dyn GfxTexture>,
        acquire_sync: &AcquireSync,
        position: Rect,
        x_off: i32,
        y_off: i32,
        size: Option<(i32, i32)>,
        capture: &ZwlrScreencopyFrameV1,
        mem: &ClientMemOffset,
        stride: i32,
        format: &'static Format,
        transform: Transform,
        scale: Scale,
    ) -> Result<(), ShmScreencopyError> {
        let (src_width, src_height) = src.size();
        let mut needs_copy = capture.rect.x1() < x_off
            || capture.rect.x2() > x_off + src_width
            || capture.rect.y1() < y_off
            || capture.rect.y2() > y_off + src_height
            || self.have_hardware_cursor();
        if let Some((target_width, target_height)) = size {
            if (target_width, target_height) != (src_width, src_height) {
                needs_copy = true;
            }
        }
        let acc = if needs_copy {
            let Some(ctx) = self.render_ctx.get() else {
                return Err(ShmScreencopyError::NoRenderContext);
            };
            let fb = ctx
                .create_fb(capture.rect.width(), capture.rect.height(), stride, format)
                .map_err(ShmScreencopyError::CreateTemporaryFb)?;
            self.perform_screencopy(
                src,
                None,
                acquire_sync,
                ReleaseSync::None,
                &fb,
                AcquireSync::Unnecessary,
                ReleaseSync::None,
                transform,
                position,
                true,
                x_off - capture.rect.x1(),
                y_off - capture.rect.y1(),
                size,
                transform,
                scale,
            )
            .map_err(ShmScreencopyError::CopyToTemporary)?;
            mem.access(|mem| {
                fb.copy_to_shm(
                    0,
                    0,
                    capture.rect.width(),
                    capture.rect.height(),
                    stride,
                    format,
                    mem,
                )
            })
        } else {
            mem.access(|mem| {
                src.clone().read_pixels(
                    capture.rect.x1() - x_off,
                    capture.rect.y1() - y_off,
                    capture.rect.width(),
                    capture.rect.height(),
                    stride,
                    format,
                    mem,
                )
            })
        };
        match acc {
            Ok(res) => res.map_err(ShmScreencopyError::ReadPixels),
            Err(e) => {
                capture.client.error(e);
                Ok(())
            }
        }
    }

    pub fn create_seat(self: &Rc<Self>, name: &str) -> Rc<WlSeatGlobal> {
        let global_name = self.globals.name();
        let seat = WlSeatGlobal::new(global_name, name, self);
        self.globals.add_global(self, &seat);
        self.ei_clients.announce_seat(&seat);
        seat
    }

    pub fn signal_point(&self, sync_obj: &SyncObj, point: SyncObjPoint) {
        let Some(ctx) = self.render_ctx.get() else {
            log::error!("Cannot signal sync obj point because there is no render context");
            return;
        };
        let Some(ctx) = ctx.sync_obj_ctx() else {
            log::error!("Cannot signal sync obj point because there is no syncobj context");
            return;
        };
        if let Err(e) = ctx.signal(sync_obj, point) {
            log::error!("Could not signal sync obj: {}", ErrorFmt(e));
        }
    }

    pub fn set_backend_idle(&self, idle: bool) {
        if self.idle.backend_idle.replace(idle) != idle {
            self.root.update_visible(self);
        }
    }

    pub fn root_visible(&self) -> bool {
        !self.idle.backend_idle.get()
    }

    pub fn find_closest_output(&self, mut x: i32, mut y: i32) -> (Rc<OutputNode>, i32, i32) {
        let mut optimal_dist = i32::MAX;
        let mut optimal_output = None;
        let outputs = self.root.outputs.lock();
        for output in outputs.values() {
            let pos = output.global.pos.get();
            let dist = pos.dist_squared(x, y);
            if dist == 0 {
                if pos.contains(x, y) {
                    return (output.clone(), x, y);
                }
            }
            if dist < optimal_dist {
                optimal_dist = dist;
                optimal_output = Some(output.clone());
            }
        }
        if let Some(output) = optimal_output {
            let pos = output.global.pos.get();
            if pos.is_empty() {
                return (output, pos.x1(), pos.y1());
            }
            if x < pos.x1() {
                x = pos.x1();
            } else if x >= pos.x2() {
                x = pos.x2() - 1;
            }
            if y < pos.y1() {
                y = pos.y1();
            } else if y >= pos.y2() {
                y = pos.y2() - 1;
            }
            return (output, x, y);
        }
        (self.dummy_output.get().unwrap(), 0, 0)
    }

    pub fn now(&self) -> Time {
        self.eng.now()
    }

    pub fn now_nsec(&self) -> u64 {
        self.eng.now().nsec()
    }

    pub fn now_usec(&self) -> u64 {
        self.eng.now().usec()
    }

    pub fn now_msec(&self) -> u64 {
        self.eng.now().msec()
    }

    pub fn output_extents_changed(&self) {
        self.root.update_extents();
        for seat in self.globals.seats.lock().values() {
            seat.output_extents_changed();
        }
    }

    pub fn update_ei_acceptor(self: &Rc<Self>) {
        self.update_ei_acceptor2();
        if let Some(forker) = self.forker.get() {
            match self.ei_acceptor.get() {
                None => {
                    forker.unsetenv(LIBEI_SOCKET.as_bytes());
                }
                Some(s) => {
                    forker.setenv(LIBEI_SOCKET.as_bytes(), s.socket_name().as_bytes());
                }
            }
        }
    }

    fn update_ei_acceptor2(self: &Rc<Self>) {
        if self.ei_acceptor.is_some() == self.enable_ei_acceptor.get() {
            return;
        }
        if self.enable_ei_acceptor.get() {
            let (acceptor, future) = match EiAcceptor::spawn(self) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("Could not create libei socket: {}", ErrorFmt(e));
                    return;
                }
            };
            self.ei_acceptor.set(Some(acceptor));
            self.ei_acceptor_future.set(Some(future));
        } else {
            log::info!("Disabling libei socket");
            self.ei_acceptor.take();
            self.ei_acceptor_future.take();
        }
    }

    pub fn vblank(&self, connector: ConnectorId) {
        if let Some(output) = self.root.outputs.get(&connector) {
            output.vblank();
        }
    }

    #[cfg(feature = "it")]
    pub async fn idle(&self) {
        loop {
            self.eng.idle().await;
            if self.cpu_worker.wait_idle() {
                break;
            }
        }
    }

    pub fn ui_drag_threshold_reached(&self, (x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> bool {
        if !self.ui_drag_enabled.get() {
            return false;
        }
        let dx = x1 - x2;
        let dy = y1 - y2;
        dx * dx + dy * dy > self.ui_drag_threshold_squared.get()
    }
}

#[derive(Debug, Error)]
pub enum ShmScreencopyError {
    #[error("There is no render context")]
    NoRenderContext,
    #[error("Could not create a bridge framebuffer")]
    CreateTemporaryFb(#[source] GfxError),
    #[error("Could not copy texture to bridge framebuffer")]
    CopyToTemporary(#[source] GfxError),
    #[error("Could not read pixels from texture")]
    ReadPixels(#[source] GfxError),
}
