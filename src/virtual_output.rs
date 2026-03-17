use {
    crate::{
        allocator::{AllocatorError, BO_USE_RENDERING, BufferObject, BufferUsage},
        async_engine::{Phase, SpawnedFuture},
        backend::{
            BackendConnectorState, Connector, ConnectorEvent, ConnectorId, ConnectorKernelId,
            DrmDeviceId, HardwareCursor, HardwareCursorUpdate, Mode, MonitorInfo,
            transaction::{
                BackendAppliedConnectorTransaction, BackendConnectorTransaction,
                BackendConnectorTransactionError, BackendConnectorTransactionType,
                BackendConnectorTransactionTypeDyn, BackendPreparedConnectorTransaction,
            },
        },
        cmm::{cmm_description::ColorDescription, cmm_primaries::Primaries},
        control_center::CCI_VIRTUAL_OUTPUTS,
        format::{Format, XRGB8888},
        gfx_api::{
            AcquireSync, BufferResv, DirectScanoutPosition, FdSync, GfxBlendBuffer, GfxContext,
            GfxError, GfxFramebuffer, GfxRenderPass, GfxTexture, ReleaseSync, create_render_pass,
        },
        ifs::{
            wl_output::{BlendSpace, OutputId},
            wp_presentation_feedback::{
                KIND_HW_CLOCK, KIND_HW_COMPLETION, KIND_VSYNC, KIND_ZERO_COPY,
            },
        },
        rect::Region,
        state::State,
        tasks::handle_connector,
        tree::OutputNode,
        utils::{
            asyncevent::AsyncEvent, cell_ext::CellExt, clonecell::CloneCell,
            copyhashmap::CopyHashMap, errorfmt::ErrorFmt, geometric_decay::GeometricDecay,
            hash_map_ext::HashMapExt, numcell::NumCell, on_change::OnChange, rc_eq::rc_eq,
            timer::TimerFd,
        },
        video::drm::ConnectorType,
    },
    ahash::AHashMap,
    linearize::{Linearize, LinearizeExt, StaticMap, static_map},
    std::{
        any::Any,
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        rc::Rc,
        time::Duration,
    },
    thiserror::Error,
    uapi::c,
};

#[derive(Default)]
pub struct VirtualOutputs {
    pub outputs: CopyHashMap<String, Rc<VirtualOutput>>,
    formats: CloneCell<Rc<Vec<&'static Format>>>,
    states: CopyHashMap<String, Rc<PersistentVirtualOutputState>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
enum FrontendState {
    #[default]
    Disconnected,
    Desktop,
    NonDesktop,
}

pub struct VirtualOutput {
    state: Rc<State>,
    id: ConnectorId,
    kernel_id: ConnectorKernelId,
    output_id: Rc<OutputId>,
    name: String,
    frontend_state: Cell<FrontendState>,
    needs_format_update: Cell<bool>,
    events: OnChange<ConnectorEvent>,
    damage: NumCell<u64>,
    present_trigger: AsyncEvent,
    persistent_state: Rc<PersistentVirtualOutputState>,
    vo_state: CloneCell<Rc<VoState>>,
    tasks: Cell<Option<[SpawnedFuture<()>; 2]>>,
    flip_task: Cell<Option<SpawnedFuture<()>>>,
    next_vblank_nsec: Cell<u64>,
    pre_commit_margin: Cell<u64>,
    pre_commit_margin_decay: GeometricDecay,
    need_vblank: AsyncEvent,
    seq: NumCell<u64>,
    pending_flip: Cell<Option<ScheduledFlip>>,
    trigger_flip: AsyncEvent,
    cursor_damage: Cell<bool>,
    cursor_programming: Cell<Option<CursorProgramming>>,
    frame_data: RefCell<Option<FrameData>>,
}

struct PersistentVirtualOutputState {
    backend_state: RefCell<BackendConnectorState>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct CursorProgramming {
    x: i32,
    y: i32,
}

struct ScheduledFlip {
    on: Rc<OutputNode>,
    refresh_ns: u64,
    vrr: bool,
    tearing: bool,
    expected_seq: Option<u64>,
    locked: bool,
    frame_data: Option<Option<FrameData>>,
}

#[derive(Default, Clone)]
struct VoState {
    fbs: Option<Rc<FbState>>,
    locked: Cell<bool>,
}

#[derive(Copy, Clone, Linearize, Eq, PartialEq)]
enum FbType {
    Primary,
    Cursor,
}

struct FbState {
    ctx: Rc<dyn GfxContext>,
    format: &'static Format,
    blend_buffer: Option<Rc<dyn GfxBlendBuffer>>,
    fbs: StaticMap<FbType, VoFb>,
}

struct VoFb {
    width: i32,
    height: i32,
    _bo: Rc<dyn BufferObject>,
    tex: Rc<dyn GfxTexture>,
    fb: Rc<dyn GfxFramebuffer>,
}

struct Transaction {
    state: Rc<State>,
    changes: AHashMap<ConnectorId, TransactionChange>,
}

struct TransactionChange {
    output: Rc<VirtualOutput>,
    new: BackendConnectorState,
}

struct PreparedTransaction {
    state: Rc<State>,
    changes: Vec<PreparedTransactionChange>,
}

struct PreparedTransactionChange {
    output: Rc<VirtualOutput>,
    old_backend_state: BackendConnectorState,
    old_vo_state: Rc<VoState>,
    new_backend_state: BackendConnectorState,
    new_vo_state: Rc<VoState>,
}

struct Latched {
    pass: GfxRenderPass,
    damage_count: u64,
    damage: Region,
    locked: bool,
}

struct CursorChange<'a> {
    swap_buffer: Option<Option<FdSync>>,
    enabled: bool,
    x: i32,
    y: i32,
    buffer: &'a VoFb,
}

struct DirectScanoutData {
    buffer_resv: Option<Rc<dyn BufferResv>>,
    tex: Rc<dyn GfxTexture>,
    acquire_sync: AcquireSync,
    release_sync: ReleaseSync,
    pos: DirectScanoutPosition,
}

struct FrameData {
    dsd: Option<DirectScanoutData>,
}

const CURSOR_SIZE: i32 = 256;

impl HardwareCursorUpdate for CursorChange<'_> {
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn get_buffer(&self) -> Rc<dyn GfxFramebuffer> {
        self.buffer.fb.clone()
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    fn swap_buffer(&mut self, sync: Option<FdSync>) {
        self.swap_buffer = Some(sync);
    }

    fn size(&self) -> (i32, i32) {
        (CURSOR_SIZE, CURSOR_SIZE)
    }
}

fn default_state(state: &State) -> BackendConnectorState {
    BackendConnectorState {
        serial: state.backend_connector_state_serials.next(),
        enabled: false,
        active: true,
        mode: Mode {
            width: 800,
            height: 600,
            refresh_rate_millihz: 60_000,
        },
        non_desktop_override: Default::default(),
        vrr: Default::default(),
        tearing: Default::default(),
        format: XRGB8888,
        color_space: Default::default(),
        eotf: Default::default(),
        gamma_lut: Default::default(),
    }
}

impl VirtualOutputs {
    pub fn get_or_create(&self, state: &Rc<State>, name: &str) -> Rc<VirtualOutput> {
        if let Some(vo) = self.outputs.get(name) {
            return vo;
        }
        let id = state.connector_ids.next();
        let kernel_id = ConnectorKernelId {
            ty: ConnectorType::VirtualOutput,
            idx: id.raw(),
        };
        let persistent_state = match self.states.get(name) {
            Some(s) => s,
            _ => {
                let state = Rc::new(PersistentVirtualOutputState {
                    backend_state: RefCell::new(default_state(state)),
                });
                self.states.set(name.to_string(), state.clone());
                state
            }
        };
        let vo = Rc::new(VirtualOutput {
            state: state.clone(),
            id,
            kernel_id,
            output_id: Rc::new(OutputId::new(
                kernel_id.to_string(),
                "Jay".to_string(),
                "VirtualOutput".to_string(),
                name.to_string(),
            )),
            name: format!("VO-{}", name),
            frontend_state: Default::default(),
            needs_format_update: Default::default(),
            events: Default::default(),
            damage: Default::default(),
            present_trigger: Default::default(),
            persistent_state,
            vo_state: Default::default(),
            tasks: Default::default(),
            flip_task: Default::default(),
            next_vblank_nsec: Default::default(),
            pre_commit_margin: Cell::new(PRE_COMMIT_MARGIN),
            pre_commit_margin_decay: GeometricDecay::new(0.5, PRE_COMMIT_MARGIN),
            need_vblank: Default::default(),
            seq: Default::default(),
            pending_flip: Default::default(),
            trigger_flip: Default::default(),
            cursor_damage: Default::default(),
            cursor_programming: Default::default(),
            frame_data: Default::default(),
        });
        vo.handle_render_ctx_change();
        handle_connector(state, &(vo.clone() as Rc<dyn Connector>));
        self.outputs.set(name.to_string(), vo.clone());
        vo.flip_task
            .set(Some(state.eng.spawn("vo-flip", vo.clone().flip_task())));
        state.trigger_cci(CCI_VIRTUAL_OUTPUTS);
        vo
    }

    pub fn remove_output(&self, state: &Rc<State>, name: &str) {
        let Some(o) = self.outputs.remove(name) else {
            return;
        };
        o.clear();
        o.events.send_event(ConnectorEvent::Disconnected);
        o.events.send_event(ConnectorEvent::Removed);
        state.trigger_cci(CCI_VIRTUAL_OUTPUTS);
    }

    pub fn clear(&self) {
        for o in self.outputs.lock().drain_values() {
            o.clear();
            o.events.clear();
        }
    }

    pub fn handle_render_ctx_change(&self, state: &State) {
        let formats = match state.render_ctx.get() {
            None => vec![],
            Some(c) => c.formats().values().map(|f| f.format).collect(),
        };
        self.formats.set(Rc::new(formats));
        for o in self.outputs.lock().values() {
            o.handle_render_ctx_change();
        }
    }
}

impl Connector for VirtualOutput {
    fn id(&self) -> ConnectorId {
        self.id
    }

    fn kernel_id(&self) -> ConnectorKernelId {
        self.kernel_id
    }

    fn event(&self) -> Option<ConnectorEvent> {
        self.events.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.events.on_change.set(Some(cb));
    }

    fn damage(&self) {
        self.damage.fetch_add(1);
        self.trigger_present();
    }

    fn drm_dev(&self) -> Option<DrmDeviceId> {
        None
    }

    fn effectively_locked(&self) -> bool {
        self.vo_state.get().locked.get()
    }

    fn state(&self) -> BackendConnectorState {
        self.persistent_state.backend_state.borrow().clone()
    }

    fn transaction_type(&self) -> Box<dyn BackendConnectorTransactionTypeDyn> {
        #[derive(Eq, PartialEq, Hash)]
        struct TT;
        impl BackendConnectorTransactionType for TT {}
        Box::new(TT)
    }

    fn create_transaction(
        &self,
    ) -> Result<Box<dyn BackendConnectorTransaction>, BackendConnectorTransactionError> {
        Ok(Box::new(self.create_transaction()))
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}

struct VirtualHc {
    o: Rc<VirtualOutput>,
}

impl Debug for VirtualHc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualOutput")
            .field("id", &self.o.id)
            .finish_non_exhaustive()
    }
}

impl HardwareCursor for VirtualHc {
    fn damage(&self) {
        self.o.cursor_damage.set(true);
        self.o.trigger_present();
    }
}

const NSEC_PER_SEC: u64 = 1_000_000_000;
const PRE_COMMIT_MARGIN: u64 = 500_000;
const PRE_COMMIT_MARGIN_DELTA: u64 = 50_000;
const POST_COMMIT_MARGIN: u64 = 500_000;

impl VirtualOutput {
    fn clear(&self) {
        self.flip_task.take();
        self.tasks.take();
        self.pending_flip.take();
        self.frame_data.take();
    }

    fn trigger_present(&self) {
        if self.pending_flip.is_none() {
            if self.cursor_damage.get() || self.damage.get() > 0 {
                self.present_trigger.trigger();
            }
        }
    }

    async fn present_task(self: Rc<Self>) {
        let be_state = self.persistent_state.backend_state.borrow().clone();
        let refresh_ns = be_state.mode.refresh_nsec();
        let vo_state = self.vo_state.get();
        let vrr = be_state.vrr;
        let tearing = be_state.tearing;
        let Some(fbs) = &vo_state.fbs else {
            return;
        };
        let mut max = 0;
        let mut cur_sec = 0;
        loop {
            self.present_trigger.triggered().await;
            if self.pending_flip.is_some() {
                continue;
            }
            let mut start = self.state.now_nsec();
            let mut expected_seq = self.seq.get() + 1;
            if !tearing {
                let next_present = self
                    .next_vblank_nsec
                    .get()
                    .saturating_sub(self.pre_commit_margin.get())
                    .saturating_sub(POST_COMMIT_MARGIN);
                if start < next_present {
                    self.state.ring.timeout(next_present).await.unwrap();
                    start = self.state.now_nsec();
                } else if !vrr {
                    expected_seq += 1;
                }
            }
            let Some(on) = self.state.root.outputs.get(&self.id) else {
                continue;
            };
            let fb = &fbs.fbs[FbType::Primary];
            let cd = on.global.color_description.get();
            let linear_cd = on.global.linear_color_description.get();
            let blend_cd = match on.global.persistent.blend_space.get() {
                BlendSpace::Linear => &linear_cd,
                BlendSpace::Srgb => self.state.color_manager.srgb_gamma22(),
            };
            let flip = match tearing {
                true => start,
                false => self.next_vblank_nsec.get().max(start),
            };
            on.before_latch(flip).await;
            if self.damage.get() > 0 || self.cursor_damage.get() {
                on.schedule.commit_cursor();
            }
            let cursor_latched = self.latch_cursor(&on, &fbs.fbs[FbType::Cursor]);
            let latched = self.latch(&on);
            on.latched(tearing);
            if latched.is_none() && cursor_latched.is_none() {
                continue;
            }
            let mut frame_data = None;
            if let Some(latched) = &latched {
                let sync;
                if let Some(dsd) = self.prepare_direct_scanout(&be_state, blend_cd, &cd, latched) {
                    sync = match dsd.acquire_sync.clone() {
                        AcquireSync::None => None,
                        AcquireSync::Implicit => None,
                        AcquireSync::FdSync(sync) => Some(sync),
                        AcquireSync::Unnecessary => None,
                    };
                    frame_data = Some(FrameData { dsd: Some(dsd) });
                } else {
                    let res = fb.fb.perform_render_pass(
                        AcquireSync::Unnecessary,
                        ReleaseSync::Explicit,
                        &cd,
                        &latched.pass,
                        &latched.damage,
                        fbs.blend_buffer.as_ref(),
                        blend_cd,
                    );
                    sync = match res {
                        Ok(sync) => sync,
                        Err(e) => {
                            log::error!("Could not present: {}", ErrorFmt(e));
                            return;
                        }
                    };
                    frame_data = Some(FrameData { dsd: None });
                };
                if let Some(sync) = sync {
                    sync.signaled(&self.state.ring, "primary").await;
                }
            }
            {
                let prev_frame_data = &*self.frame_data.borrow();
                let effective_frame_data = frame_data.as_ref().or(prev_frame_data.as_ref());
                if let Some(fd) = effective_frame_data {
                    match &fd.dsd {
                        None => {
                            on.perform_screencopies(
                                &fb.tex,
                                &cd,
                                None,
                                &AcquireSync::Unnecessary,
                                ReleaseSync::None,
                                true,
                                0,
                                0,
                                None,
                            );
                        }
                        Some(dsd) => {
                            on.perform_screencopies(
                                &dsd.tex,
                                &cd,
                                dsd.buffer_resv.as_ref(),
                                &dsd.acquire_sync,
                                dsd.release_sync,
                                true,
                                dsd.pos.crtc_x,
                                dsd.pos.crtc_y,
                                Some((dsd.pos.crtc_width, dsd.pos.crtc_height)),
                            );
                        }
                    }
                }
            }
            if let Some(Some(sync)) = cursor_latched {
                sync.signaled(&self.state.ring, "cursor").await;
            }
            if let Some(latched) = &latched {
                vo_state.locked.set(latched.locked);
                self.damage.fetch_sub(latched.damage_count);
            }
            self.pending_flip.set(Some(ScheduledFlip {
                on,
                refresh_ns,
                vrr,
                tearing,
                expected_seq: (!tearing).then_some(expected_seq),
                locked: vo_state.locked.get(),
                frame_data: frame_data.map(Some),
            }));
            if vrr {
                self.need_vblank.trigger();
            }
            if tearing {
                self.trigger_flip.trigger();
            }
            let duration = self.state.now_nsec() - start;
            max = max.max(duration);
            if start / NSEC_PER_SEC != cur_sec {
                cur_sec = start / NSEC_PER_SEC;
                self.pre_commit_margin_decay.add(max);
                self.pre_commit_margin
                    .set(self.pre_commit_margin_decay.get());
                max = 0;
            }
        }
    }

    fn latch(&self, on: &Rc<OutputNode>) -> Option<Latched> {
        let damage_count = self.damage.get();
        if damage_count == 0 {
            return None;
        }
        let damage = {
            on.global.connector.damaged.set(false);
            on.global.add_visualizer_damage();
            let damage = &mut *on.global.connector.damage.borrow_mut();
            let region = Region::from_rects2(damage);
            damage.clear();
            region
        };
        let pass = create_render_pass(
            on.global.mode.get().size(),
            &**on,
            &self.state,
            Some(on.global.pos.get()),
            on.global.persistent.scale.get(),
            true,
            false,
            on.has_fullscreen(),
            true,
            on.global.persistent.transform.get(),
            Some(&self.state.damage_visualizer),
        );
        Some(Latched {
            pass,
            damage_count,
            damage,
            locked: self.state.lock.locked.get(),
        })
    }

    fn prepare_direct_scanout(
        &self,
        be_state: &BackendConnectorState,
        blend_cd: &Rc<ColorDescription>,
        cd: &Rc<ColorDescription>,
        latched: &Latched,
    ) -> Option<DirectScanoutData> {
        let (ct, position) = latched.pass.prepare_direct_scanout(
            be_state.mode.width,
            be_state.mode.height,
            blend_cd,
            &cd,
            true,
        )?;
        Some(DirectScanoutData {
            buffer_resv: ct.buffer_resv.clone(),
            tex: ct.tex.clone(),
            acquire_sync: ct.acquire_sync.clone(),
            release_sync: ct.release_sync,
            pos: position,
        })
    }

    fn latch_cursor(&self, on: &Rc<OutputNode>, fb: &VoFb) -> Option<Option<FdSync>> {
        if !self.cursor_damage.take() {
            return None;
        }
        let mut c = CursorChange {
            enabled: false,
            swap_buffer: None,
            x: 0,
            y: 0,
            buffer: fb,
        };
        if let Some(p) = self.cursor_programming.get() {
            c.enabled = true;
            c.x = p.x;
            c.y = p.y;
        }
        self.state.present_hardware_cursor(on, &mut c);
        let p = c.enabled.then_some(CursorProgramming { x: c.x, y: c.y });
        let mut cursor_changed = false;
        cursor_changed |= self.cursor_programming.replace(p) != p;
        cursor_changed |= c.swap_buffer.is_some();
        cursor_changed.then_some(c.swap_buffer.take().flatten())
    }

    async fn vblank_task(self: Rc<Self>) {
        let be_state = self.persistent_state.backend_state.borrow().clone();
        let refresh_nsec = be_state.mode.refresh_nsec();
        let vrr = be_state.vrr;
        let handle_vblank = || {
            let next_vblank = self.state.now_nsec().saturating_add(refresh_nsec);
            self.next_vblank_nsec.set(next_vblank);
            self.seq.fetch_add(1);
            if self.pending_flip.is_some() {
                self.trigger_flip.trigger();
            }
            if let Some(on) = self.state.root.outputs.get(&self.id) {
                on.vblank();
            }
            next_vblank
        };
        if vrr {
            loop {
                let next_vblank = handle_vblank();
                if let Err(e) = self.state.ring.timeout(next_vblank).await {
                    log::error!("Could not wait for next vblank: {}", e);
                    return;
                }
                self.need_vblank.triggered().await;
            }
        } else {
            let tfd = match TimerFd::new(c::CLOCK_MONOTONIC) {
                Ok(fd) => fd,
                Err(e) => {
                    log::error!("Could not create a timer fd: {}", ErrorFmt(e));
                    return;
                }
            };
            let duration = Some(Duration::from_nanos(refresh_nsec));
            let res = tfd.program(duration, duration);
            if let Err(e) = res {
                log::error!("Could not program the timer fd: {}", ErrorFmt(e));
                return;
            }
            loop {
                handle_vblank();
                if let Err(e) = tfd.expired(&self.state.ring).await {
                    log::error!("Could not wait for timer fd to expire: {}", ErrorFmt(e));
                    return;
                }
            }
        }
    }

    async fn flip_task(self: Rc<Self>) {
        let debounce = self.state.ring.debouncer(0);
        loop {
            self.trigger_flip.triggered().await;
            let Some(mut flip) = self.pending_flip.take() else {
                continue;
            };
            let direct_scanout = {
                let fd = &mut *self.frame_data.borrow_mut();
                if let Some(frame) = flip.frame_data.take() {
                    *fd = frame;
                }
                matches!(*fd, Some(FrameData { dsd: Some(..), .. }))
            };
            let flip_ns = self.state.now_nsec();
            let tv_sec = flip_ns / NSEC_PER_SEC;
            let tv_nsec = (flip_ns % NSEC_PER_SEC) as u32;
            let mut flags = KIND_HW_COMPLETION | KIND_HW_CLOCK;
            if !flip.tearing {
                flags |= KIND_VSYNC;
            }
            if direct_scanout {
                flags |= KIND_ZERO_COPY;
            }
            let seq = self.seq.get();
            flip.on.presented(
                tv_sec,
                tv_nsec,
                flip.refresh_ns.try_into().unwrap_or(0),
                seq,
                flags,
                flip.vrr,
                flip.locked,
            );
            self.trigger_present();
            if let Some(expected_seq) = flip.expected_seq
                && seq > expected_seq
            {
                let mut margin = self.pre_commit_margin.get();
                if margin < flip.refresh_ns {
                    margin += PRE_COMMIT_MARGIN_DELTA;
                    self.pre_commit_margin.set(margin);
                    self.pre_commit_margin_decay.reset(margin);
                }
            }
            debounce.debounce().await;
        }
    }

    fn create_transaction(&self) -> Transaction {
        Transaction {
            state: self.state.clone(),
            changes: Default::default(),
        }
    }

    fn handle_render_ctx_change(self: &Rc<Self>) {
        self.needs_format_update.set(true);
        self.reapply_state();
        self.notify_frontend();
    }

    fn reapply_state(self: &Rc<Self>) {
        let Err(e) = self.reapply_state_() else {
            return;
        };
        log::error!("Could not reapply state: {}", ErrorFmt(e));
        let retry = {
            let bs = &mut *self.persistent_state.backend_state.borrow_mut();
            mem::replace(&mut bs.format, XRGB8888) != XRGB8888
        };
        if retry {
            log::info!("Retrying with format {}", XRGB8888.name);
            let Err(e) = self.reapply_state_() else {
                return;
            };
            log::error!("Could not reapply state: {}", ErrorFmt(e));
        }
        let retry = {
            let def = default_state(&self.state);
            let bs = &mut *self.persistent_state.backend_state.borrow_mut();
            mem::replace(bs, def.clone()) != def
        };
        if retry {
            log::info!("Retrying with default state");
            let Err(e) = self.reapply_state_() else {
                return;
            };
            log::error!("Could not reapply state: {}", ErrorFmt(e));
        }
        self.tasks.take();
        self.vo_state.take();
    }

    fn reapply_state_(self: &Rc<Self>) -> Result<(), BackendConnectorTransactionError> {
        let mut transaction = self.create_transaction();
        transaction.add(self, self.persistent_state.backend_state.borrow().clone())?;
        transaction.prepare()?.apply();
        Ok(())
    }

    fn notify_frontend(self: &Rc<Self>) {
        let state = self.persistent_state.backend_state.borrow().clone();
        let desired_state = match state.enabled {
            true => FrontendState::Desktop,
            false => FrontendState::NonDesktop,
        };
        let current_state = self.frontend_state.get();
        if desired_state != current_state {
            if current_state != FrontendState::Disconnected {
                self.events.send_event(ConnectorEvent::Disconnected);
            }
            self.events
                .send_event(ConnectorEvent::Connected(MonitorInfo {
                    modes: None,
                    output_id: self.output_id.clone(),
                    width_mm: Default::default(),
                    height_mm: Default::default(),
                    non_desktop: Default::default(),
                    non_desktop_effective: !state.enabled,
                    vrr_capable: true,
                    eotfs: LinearizeExt::variants()
                        .filter(|v| *v != Default::default())
                        .collect(),
                    color_spaces: LinearizeExt::variants()
                        .filter(|v| *v != Default::default())
                        .collect(),
                    primaries: Primaries::SRGB,
                    luminance: Default::default(),
                    state: state.clone(),
                }));
            if state.enabled {
                self.needs_format_update.set(true);
                let hc = Rc::new(VirtualHc { o: self.clone() });
                self.events
                    .send_event(ConnectorEvent::HardwareCursor(Some(hc)));
            }
        }
        if state.enabled && self.needs_format_update.take() {
            self.events.send_event(ConnectorEvent::FormatsChanged(
                self.state.virtual_outputs.formats.get(),
            ));
        }
        self.frontend_state.set(desired_state);
    }
}

impl Transaction {
    fn add(
        &mut self,
        connector: &Rc<VirtualOutput>,
        change: BackendConnectorState,
    ) -> Result<(), BackendConnectorTransactionError> {
        if change.mode.width <= 0 || change.mode.height <= 0 {
            return Err(BackendConnectorTransactionError::UnsupportedMode(
                connector.kernel_id(),
                change.mode,
            ));
        }
        if change.gamma_lut.is_some() {
            return Err(BackendConnectorTransactionError::GammaLutNotSupported(
                connector.kernel_id(),
            ));
        }
        self.changes.insert(
            connector.id,
            TransactionChange {
                output: connector.clone(),
                new: change,
            },
        );
        Ok(())
    }

    fn prepare(&mut self) -> Result<PreparedTransaction, BackendConnectorTransactionError> {
        let mut changes = vec![];
        let ctx = self.state.render_ctx.get();
        for change in self.changes.drain_values() {
            let old_backend_state = change
                .output
                .persistent_state
                .backend_state
                .borrow()
                .clone();
            let old_vo_state = change.output.vo_state.get();
            let mut new_vo_state = (*old_vo_state).clone();
            let mode = change.new.mode;
            'discard_fbs: {
                if let Some(fbs) = &new_vo_state.fbs {
                    macro_rules! discard {
                        () => {
                            new_vo_state.fbs = None;
                            break 'discard_fbs;
                        };
                    }
                    if !change.new.enabled {
                        discard!();
                    }
                    let Some(ctx) = &ctx else {
                        discard!();
                    };
                    if !rc_eq(&fbs.ctx, ctx) {
                        discard!();
                    }
                    if fbs.format != change.new.format {
                        discard!();
                    }
                    let fb = &fbs.fbs[FbType::Primary];
                    if (fb.width, fb.height) != mode.size() {
                        discard!();
                    }
                }
            }
            if new_vo_state.fbs.is_none() {
                new_vo_state.locked.set(true);
            }
            if change.new.enabled && new_vo_state.fbs.is_none() {
                let Some(ctx) = &ctx else {
                    return Err(BackendConnectorTransactionError::NoRenderContext);
                };
                let bb = match ctx.acquire_blend_buffer(mode.width, mode.height) {
                    Ok(bb) => Some(bb),
                    Err(e) => {
                        log::warn!("Could not create a blend buffer: {}", ErrorFmt(e));
                        None
                    }
                };
                let sizes = static_map! {
                    FbType::Cursor => (CURSOR_SIZE, CURSOR_SIZE),
                    FbType::Primary => mode.size(),
                };
                let fbs = allocate_scanout_buffers(&self.state, ctx, change.new.format, sizes)
                    .map_err(|e| {
                        BackendConnectorTransactionError::AllocateScanoutBuffers(
                            change.output.kernel_id(),
                            Box::new(e),
                        )
                    })?;
                new_vo_state.fbs = Some(Rc::new(FbState {
                    ctx: ctx.clone(),
                    format: change.new.format,
                    blend_buffer: bb,
                    fbs,
                }));
            }
            changes.push(PreparedTransactionChange {
                output: change.output,
                old_backend_state,
                old_vo_state,
                new_backend_state: change.new,
                new_vo_state: Rc::new(new_vo_state),
            });
        }
        Ok(PreparedTransaction {
            state: self.state.clone(),
            changes,
        })
    }
}

impl PreparedTransaction {
    fn apply(&mut self) {
        let eng = &self.state.eng;
        for change in &mut self.changes {
            let o = &change.output;
            let mut tasks = None;
            let ns = &change.new_backend_state;
            if ns.enabled && ns.active {
                tasks = Some([
                    eng.spawn2("vo-present", Phase::Present, o.clone().present_task()),
                    eng.spawn("vo-vblank", o.clone().vblank_task()),
                ]);
                o.damage();
                if let Some(on) = self.state.root.outputs.get(&o.id) {
                    on.global.add_damage_area(&on.global.pos.get());
                    on.global.connector.damage();
                }
            } else {
                if let Some(mut flip) = o.pending_flip.take() {
                    flip.frame_data = Some(None);
                    o.pending_flip.set(Some(flip));
                } else {
                    o.frame_data.take();
                }
            }
            o.tasks.set(tasks);
            o.trigger_flip.trigger();
            *o.persistent_state.backend_state.borrow_mut() = ns.clone();
            o.vo_state.set(change.new_vo_state.clone());
            o.notify_frontend();
            mem::swap(&mut change.new_vo_state, &mut change.old_vo_state);
            mem::swap(&mut change.new_backend_state, &mut change.old_backend_state);
        }
    }
}

impl BackendConnectorTransaction for Transaction {
    fn add(
        &mut self,
        connector: &Rc<dyn Connector>,
        change: BackendConnectorState,
    ) -> Result<(), BackendConnectorTransactionError> {
        let Ok(connector) = (connector.clone() as Rc<dyn Any>).downcast::<VirtualOutput>() else {
            return Err(BackendConnectorTransactionError::UnsupportedConnectorType(
                connector.kernel_id(),
            ));
        };
        self.add(&connector, change)
    }

    fn prepare(
        mut self: Box<Self>,
    ) -> Result<Box<dyn BackendPreparedConnectorTransaction>, BackendConnectorTransactionError>
    {
        (*self).prepare().map(|t| Box::new(t) as _)
    }
}

impl BackendPreparedConnectorTransaction for PreparedTransaction {
    fn apply(
        mut self: Box<Self>,
    ) -> Result<Box<dyn BackendAppliedConnectorTransaction>, BackendConnectorTransactionError> {
        (*self).apply();
        Ok(self)
    }
}

impl BackendAppliedConnectorTransaction for PreparedTransaction {
    fn commit(self: Box<Self>) {
        // nothing
    }

    fn rollback(mut self: Box<Self>) -> Result<(), BackendConnectorTransactionError> {
        (*self).apply();
        Ok(())
    }
}

#[derive(Debug, Error)]
enum AllocError {
    #[error("GfxContext does not support the format")]
    GfxFormatNotSupported,
    #[error("Could not allocate the BO")]
    CreateBo(#[source] AllocatorError),
    #[error("Could not import the dmabuf into the GfxContext")]
    ImportImage(#[source] GfxError),
    #[error("Could not create a texture")]
    CreateTexture(#[source] GfxError),
    #[error("Could not create a framebuffer")]
    CreateFb(#[source] GfxError),
}

fn allocate_scanout_buffers(
    state: &Rc<State>,
    ctx: &Rc<dyn GfxContext>,
    format: &'static Format,
    sizes: StaticMap<FbType, (i32, i32)>,
) -> Result<StaticMap<FbType, VoFb>, AllocError> {
    let Some(gfx_format) = ctx.formats().get(&format.drm) else {
        return Err(AllocError::GfxFormatNotSupported);
    };
    let mut needs_render_usage = false;
    let mut modifiers = vec![];
    for modifier in gfx_format.read_modifiers.iter().copied() {
        let Some(write_modifier) = gfx_format.write_modifiers.get(&modifier) else {
            continue;
        };
        needs_render_usage |= write_modifier.needs_render_usage;
        modifiers.push(modifier);
    }
    let mut usage = BufferUsage::none();
    if needs_render_usage {
        usage |= BO_USE_RENDERING;
    }
    let create_fb = |(width, height): (i32, i32)| {
        let bo = ctx
            .allocator()
            .create_bo(&state.dma_buf_ids, width, height, format, &modifiers, usage)
            .map_err(AllocError::CreateBo)?;
        let img = ctx
            .clone()
            .dmabuf_img(bo.dmabuf())
            .map_err(AllocError::ImportImage)?;
        let tex = img
            .clone()
            .to_texture()
            .map_err(AllocError::CreateTexture)?;
        let fb = img.clone().to_framebuffer().map_err(AllocError::CreateFb)?;
        Ok(VoFb {
            width,
            height,
            _bo: bo,
            tex,
            fb,
        })
    };
    let fbs = static_map! {
        t => create_fb(sizes[t])?,
    };
    Ok(fbs)
}
