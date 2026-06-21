use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        client::Client,
        copy_device::CopyDevice,
        gfx_api::{
            AsyncShmGfxTextureCallback, GfxContext, GfxError, PendingShmTransfer, STAGING_UPLOAD,
        },
        ifs::{
            wl_buffer::WlBufferStorage,
            wl_surface::{PendingState, WlSurface, WlSurfaceError, prime::PrimeValidity},
        },
        io_uring::{
            IoUring, IoUringError, PendingPoll, PendingTimeout, PollCallback, TimeoutCallback,
        },
        syncobj::{
            SyncobjError,
            wait_for_syncobj::{SyncobjWaiter, WaitForSyncobj, WaitForSyncobjHandle},
        },
        tree::{BeforeLatchResult, TreeSerial},
        utils::{
            box_cache::{BoxCache, BoxReset, BoxUninit, CachedBox},
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
            numcell::NumCell,
            obj_and_id::{ObjAndId, ObjWithId},
            oserror::OsError,
            queue::AsyncQueue,
            syncqueue::SyncQueue,
        },
        video::{
            dmabuf::{ChainedCopyCallback, ChainedCopyError, PendingChainedCopy},
            drm::syncobj::{Syncobj, SyncobjPoint},
        },
    },
    isnt::std_1::{primitive::IsntSliceExt, vec::IsntVecExt},
    smallvec::SmallVec,
    std::{
        cell::{Cell, LazyCell, RefCell},
        ops::DerefMut,
        rc::{Rc, Weak},
        slice,
    },
    thiserror::Error,
    uapi::{OwnedFd, c::c_short},
};

const MAX_TIMELINE_DEPTH: usize = 256;

linear_ids!(CommitTimelineIds, CommitTimelineId, u64);

pub struct CommitTimelines {
    next_id: CommitTimelineIds,
    wfs: Rc<WaitForSyncobj>,
    ring: Rc<IoUring>,
    depth: NumCell<usize>,
    gc: CopyHashMap<CommitTimelineId, Rc<Inner>>,
    flush_requests: Rc<FlushRequests>,
    _flush_requests_future: SpawnedFuture<()>,
}

struct CommitTimeWaiter {
    node: Rc<Entry>,
    present: u64,
}

pub struct CommitTimeline {
    shared: Rc<CommitTimelines>,
    own_timeline: Rc<Inner>,
    effective_timeline: ObjAndId<Rc<Inner>>,
    fifo_barrier_set: Cell<bool>,
    fifo_waiter: Cell<Option<Rc<Inner>>>,
    commit_time_waiter: RefCell<Option<CommitTimeWaiter>>,
    toplevel_restored_waiter: Cell<Option<Rc<Inner>>>,
    serial_waiter: Cell<Option<Rc<Inner>>>,
}

#[derive(Default)]
pub struct CommitCache {
    cache: Rc<BoxCache<Commit, BoxUninit>>,
}

struct Inner {
    id: CommitTimelineId,
    entries: SyncQueue<Rc<Entry>>,
}

impl ObjWithId for Rc<Inner> {
    type Id = CommitTimelineId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

fn add_entry(inner: &Rc<Inner>, shared: &Rc<CommitTimelines>, kind: EntryKind) -> Rc<Entry> {
    shared.depth.fetch_add(1);
    let entry = Rc::new(Entry {
        inner: inner.clone(),
        shared: shared.clone(),
        kind,
    });
    inner.entries.push(entry.clone());
    entry
}

#[derive(Debug, Error)]
pub enum CommitTimelineError {
    #[error(transparent)]
    ImmediateCommit(WlSurfaceError),
    #[error("Could not apply a delayed commit")]
    DelayedCommit(#[source] WlSurfaceError),
    #[error("Could not register a wait")]
    RegisterWait(#[source] SyncobjError),
    #[error("Syncobj wait failed")]
    Wait(#[source] SyncobjError),
    #[error("The client has too many pending commits")]
    Depth,
    #[error("Could not upload a shm texture")]
    ShmUpload(#[source] GfxError),
    #[error("Could not register an implicit-sync wait")]
    RegisterImplicitPoll(#[source] IoUringError),
    #[error("Could not wait for a dmabuf to become idle")]
    PollDmabuf(#[source] OsError),
    #[error("Could not wait for the commit timeout")]
    CommitTimeout(#[source] OsError),
    #[error("Could not perform prime copy")]
    PrimeCopy(#[source] ChainedCopyError),
}

impl CommitTimelines {
    pub fn new(
        wfs: &Rc<WaitForSyncobj>,
        ring: &Rc<IoUring>,
        eng: &Rc<AsyncEngine>,
        client: &Weak<Client>,
    ) -> Self {
        let flush_requests = Rc::new(FlushRequests::default());
        let flush_request_future = eng.spawn(
            "wl_surface flush requests",
            process_flush_requests(client.clone(), flush_requests.clone()),
        );
        Self {
            flush_requests,
            _flush_requests_future: flush_request_future,
            next_id: Default::default(),
            depth: NumCell::new(0),
            wfs: wfs.clone(),
            gc: Default::default(),
            ring: ring.clone(),
        }
    }

    pub fn create_timeline(self: &Rc<Self>) -> CommitTimeline {
        let id = self.next_id.next();
        let timeline = Rc::new(Inner {
            id,
            entries: Default::default(),
        });
        CommitTimeline {
            shared: self.clone(),
            own_timeline: timeline.clone(),
            effective_timeline: ObjAndId::new(timeline),
            fifo_barrier_set: Cell::new(false),
            fifo_waiter: Default::default(),
            commit_time_waiter: Default::default(),
            toplevel_restored_waiter: Default::default(),
            serial_waiter: Default::default(),
        }
    }

    pub fn clear(&self) {
        self.flush_requests.flush_waiters.clear();
        for list in self.gc.lock().drain_values() {
            break_loops(&list);
        }
    }
}

pub enum ClearReason {
    BreakLoops,
    Destroy,
}

fn break_loops(inner: &Rc<Inner>) {
    while let Some(entry) = inner.entries.pop() {
        if let EntryKind::Commit(c) = &entry.kind {
            c.wait_handles.take();
            *c.shm_upload.borrow_mut() = ShmUploadState::None;
            c.prime_copies.take();
            c.prime_validities.take();
            c.pending_polls.take();
            *c.commit_times.borrow_mut() = CommitTimesState::Ready;
        }
    }
}

impl CommitTimeline {
    pub fn clear(&self, reason: ClearReason) {
        match reason {
            ClearReason::BreakLoops => {
                self.fifo_waiter.take();
                self.commit_time_waiter.take();
                self.toplevel_restored_waiter.take();
                self.serial_waiter.take();
                break_loops(&self.own_timeline)
            }
            ClearReason::Destroy => {
                self.clear_fifo_barrier();
                if self.own_timeline.entries.is_not_empty() {
                    add_entry(
                        &self.own_timeline,
                        &self.shared,
                        EntryKind::Gc(self.own_timeline.id),
                    );
                    self.shared
                        .gc
                        .set(self.own_timeline.id, self.own_timeline.clone());
                }
            }
        }
    }

    pub(super) fn commit(
        &self,
        surface: &Rc<WlSurface>,
        pending: &mut CachedBox<PendingState, BoxReset>,
    ) -> Result<(), CommitTimelineError> {
        let state = &surface.client.state;
        let mut collector = CommitDataCollector {
            render_ctx: LazyCell::new(|| {
                let ctx = state.render_ctx.get()?;
                Some((ctx, state.render_ctx_prime_copy_device.get()))
            }),
            acquire_points: Default::default(),
            shm_uploads: 0,
            has_dmabuf: Default::default(),
            needs_prime_copies: Default::default(),
            acquire_files: Default::default(),
            commit_time: Default::default(),
            toplevel_restored: Default::default(),
        };
        collector.collect(pending);
        let points = collector.acquire_points;
        let pending_uploads = collector.shm_uploads;
        let has_dmabuf = collector.has_dmabuf;
        let needs_prime_copies = collector.needs_prime_copies;
        let acquire_files = collector.acquire_files;
        let commit_time = collector.commit_time;
        let toplevel_restored = collector.toplevel_restored;
        let has_commit_time = commit_time > 0;
        let serial = pending.serial;
        let has_dependencies = points.is_not_empty()
            || pending_uploads > 0
            || needs_prime_copies
            || acquire_files.is_not_empty()
            || has_commit_time
            || toplevel_restored.is_some()
            || serial.is_some();
        let must_be_queued = has_dependencies
            || self.own_timeline.entries.is_not_empty()
            || (pending.fifo_barrier_wait && self.fifo_barrier_set.get());
        if !must_be_queued {
            return surface
                .apply_state(pending)
                .map_err(CommitTimelineError::ImmediateCommit);
        }
        if self.shared.depth.get() >= MAX_TIMELINE_DEPTH {
            return Err(CommitTimelineError::Depth);
        }
        set_effective_timeline(self, pending, &self.own_timeline);
        let queue_was_empty = self.own_timeline.entries.is_empty();
        let commit_fifo_state = match pending.fifo_barrier_wait {
            true => CommitFifoState::Queued,
            false => CommitFifoState::Mailbox,
        };
        let entry = add_entry(
            &self.own_timeline,
            &self.shared,
            EntryKind::Commit(surface.client.state.commit_cache.cache.get(Commit {
                surface: surface.clone(),
                pending: RefCell::new(CachedBox::take(pending)),
                syncobj: NumCell::new(points.len()),
                wait_handles: Cell::new(Default::default()),
                pending_uploads: NumCell::new(pending_uploads),
                shm_upload: RefCell::new(ShmUploadState::None),
                has_dmabuf,
                pending_prime_copies: Default::default(),
                prime_copies: Default::default(),
                prime_validities: Default::default(),
                num_pending_polls: NumCell::new(acquire_files.len()),
                pending_polls: Cell::new(Default::default()),
                fifo_state: Cell::new(commit_fifo_state),
                commit_times: RefCell::new(CommitTimesState::Ready),
                toplevel_restored,
                serial,
            })),
        );
        let mut needs_flush = commit_fifo_state == CommitFifoState::Queued;
        if has_dependencies {
            let EntryKind::Commit(commit) = &entry.kind else {
                unreachable!();
            };
            if points.is_not_empty() {
                let mut wait_handles = SmallVec::new();
                for (syncobj, point) in points {
                    let handle = self
                        .shared
                        .wfs
                        .wait(&syncobj, point, true, entry.clone())
                        .map_err(CommitTimelineError::RegisterWait)?;
                    wait_handles.push(handle);
                }
                commit.wait_handles.set(wait_handles);
            }
            if pending_uploads > 0 {
                *commit.shm_upload.borrow_mut() = ShmUploadState::Todo;
                needs_flush = true;
            }
            if needs_prime_copies {
                needs_flush = true;
            }
            if acquire_files.is_not_empty() {
                let mut pending_polls = SmallVec::new();
                for fd in acquire_files {
                    let handle = self
                        .shared
                        .ring
                        .readable_external(&fd, entry.clone())
                        .map_err(CommitTimelineError::RegisterImplicitPoll)?;
                    pending_polls.push(handle);
                }
                commit.pending_polls.set(pending_polls);
            }
            if has_commit_time {
                *commit.commit_times.borrow_mut() = CommitTimesState::Queued { time: commit_time };
                needs_flush = true;
            }
            if commit.toplevel_restored.is_some() {
                needs_flush = true;
            }
            if serial.is_some() {
                needs_flush = true;
            }
        }
        if needs_flush && queue_was_empty {
            flush_list(&self.own_timeline).map_err(CommitTimelineError::DelayedCommit)?;
        }
        Ok(())
    }

    pub fn set_fifo_barrier(&self) {
        self.fifo_barrier_set.set(true);
    }

    pub fn clear_fifo_barrier(&self) {
        self.fifo_barrier_set.set(false);
        if let Some(waiter) = self.fifo_waiter.take() {
            self.shared.flush_requests.flush_waiters.push(waiter);
        }
    }

    pub fn has_fifo_barrier(&self) -> bool {
        self.fifo_barrier_set.get()
    }

    pub fn toplevel_restored(&self) {
        if let Some(waiter) = self.toplevel_restored_waiter.take() {
            self.shared.flush_requests.flush_waiters.push(waiter);
        }
    }

    pub fn serial_unblocked(&self) {
        if let Some(waiter) = self.serial_waiter.take() {
            self.shared.flush_requests.flush_waiters.push(waiter);
        }
    }

    pub fn before_latch(&self, surface: &WlSurface, present: u64) -> BeforeLatchResult {
        let waiter = &mut *self.commit_time_waiter.borrow_mut();
        if let Some(w) = waiter {
            if w.present <= present {
                let EntryKind::Commit(c) = &w.node.kind else {
                    unreachable!();
                };
                *c.commit_times.borrow_mut() = CommitTimesState::Ready;
                self.shared
                    .flush_requests
                    .flush_waiters
                    .push(w.node.inner.clone());
                *waiter = None;
                surface.before_latch_listener.detach();
                BeforeLatchResult::Yield
            } else {
                BeforeLatchResult::None
            }
        } else {
            surface.before_latch_listener.detach();
            BeforeLatchResult::None
        }
    }
}

impl SyncobjWaiter for Entry {
    fn done(self: Rc<Self>, result: Result<(), SyncobjError>) {
        let EntryKind::Commit(commit) = &self.kind else {
            unreachable!();
        };
        if let Err(e) = result {
            commit.surface.client.error(CommitTimelineError::Wait(e));
            return;
        }
        commit.syncobj.fetch_sub(1);
        flush_commit(&self.inner, commit);
    }
}

fn flush_commit(list: &Rc<Inner>, commit: &Commit) {
    if let Err(e) = flush_list(list) {
        commit
            .surface
            .client
            .error(CommitTimelineError::DelayedCommit(e));
    }
}

impl AsyncShmGfxTextureCallback for Entry {
    fn completed(self: Rc<Self>, res: Result<(), GfxError>) {
        let EntryKind::Commit(commit) = &self.kind else {
            unreachable!();
        };
        if let Err(e) = res {
            commit
                .surface
                .client
                .error(CommitTimelineError::ShmUpload(e));
            return;
        }
        commit.pending_uploads.fetch_sub(1);
        flush_commit(&self.inner, commit);
    }
}

impl ChainedCopyCallback for Entry {
    fn completed(self: Rc<Self>, res: Result<(), ChainedCopyError>) {
        let EntryKind::Commit(commit) = &self.kind else {
            unreachable!();
        };
        if let Err(e) = res {
            commit
                .surface
                .client
                .error(CommitTimelineError::PrimeCopy(e));
            return;
        }
        commit.pending_prime_copies.fetch_sub(1);
        flush_commit(&self.inner, commit);
    }
}

impl PollCallback for Entry {
    fn completed(self: Rc<Self>, res: Result<c_short, OsError>) {
        let EntryKind::Commit(commit) = &self.kind else {
            unreachable!();
        };
        if let Err(e) = res {
            commit
                .surface
                .client
                .error(CommitTimelineError::PollDmabuf(e));
            return;
        }
        commit.num_pending_polls.fetch_sub(1);
        flush_commit(&self.inner, commit);
    }
}

impl TimeoutCallback for Entry {
    fn completed(self: Rc<Self>, res: Result<(), OsError>, _data: u64) {
        let EntryKind::Commit(commit) = &self.kind else {
            unreachable!();
        };
        commit.surface.commit_timeline.commit_time_waiter.take();
        commit.surface.before_latch_listener.detach();
        if let Err(e) = res {
            commit
                .surface
                .client
                .error(CommitTimelineError::CommitTimeout(e));
            return;
        }
        *commit.commit_times.borrow_mut() = CommitTimesState::Ready;
        flush_commit(&self.inner, commit);
    }
}

struct Entry {
    inner: Rc<Inner>,
    shared: Rc<CommitTimelines>,
    kind: EntryKind,
}

enum EntryKind {
    Commit(CachedBox<Commit, BoxUninit>),
    Wait(Rc<Cell<bool>>),
    Signal(Rc<Inner>, Rc<Cell<bool>>),
    Gc(CommitTimelineId),
}

enum ShmUploadState {
    None,
    Todo,
    Scheduled(#[expect(dead_code)] SmallVec<[PendingShmTransfer; 1]>),
}

enum CommitTimesState {
    Ready,
    Queued { time: u64 },
    Registered { _pending: PendingTimeout },
}

struct Commit {
    surface: Rc<WlSurface>,
    pending: RefCell<CachedBox<PendingState, BoxReset>>,
    syncobj: NumCell<usize>,
    wait_handles: Cell<SmallVec<[WaitForSyncobjHandle; 1]>>,
    pending_uploads: NumCell<usize>,
    shm_upload: RefCell<ShmUploadState>,
    has_dmabuf: bool,
    pending_prime_copies: NumCell<usize>,
    prime_copies: Cell<Option<SmallVec<[PendingChainedCopy; 1]>>>,
    prime_validities: RefCell<Option<SmallVec<[PrimeValidity; 1]>>>,
    num_pending_polls: NumCell<usize>,
    pending_polls: Cell<SmallVec<[PendingPoll; 1]>>,
    fifo_state: Cell<CommitFifoState>,
    commit_times: RefCell<CommitTimesState>,
    toplevel_restored: Option<Rc<Cell<bool>>>,
    serial: Option<TreeSerial>,
}

fn flush_list(inner: &Rc<Inner>) -> Result<(), WlSurfaceError> {
    while let Some(el) = inner.entries.pop() {
        if el.maybe_apply()? {
            el.shared.depth.fetch_sub(1);
        } else {
            inner.entries.push_front(el);
            break;
        }
    }
    Ok(())
}

impl Entry {
    fn maybe_apply(self: &Rc<Self>) -> Result<bool, WlSurfaceError> {
        match &self.kind {
            EntryKind::Commit(c) => {
                let mut has_unmet_dependencies = false;
                let may_access_buffer = c.syncobj.get() == 0 && c.num_pending_polls.get() == 0;
                if may_access_buffer {
                    if c.pending_uploads.get() > 0 {
                        check_shm_uploads(self, c)?;
                        has_unmet_dependencies |= c.pending_uploads.get() > 0;
                    }
                    if c.has_dmabuf && c.pending_prime_copies.get() == 0 {
                        check_prime_copies(self, c)?;
                    }
                    has_unmet_dependencies |= c.pending_prime_copies.get() > 0;
                } else {
                    has_unmet_dependencies = true;
                }
                let tl = &c.surface.commit_timeline;
                if tl.fifo_barrier_set.get() {
                    match c.fifo_state.get() {
                        CommitFifoState::Queued => {
                            tl.fifo_waiter.set(Some(self.inner.clone()));
                            c.fifo_state.set(CommitFifoState::Registered);
                            has_unmet_dependencies = true;
                        }
                        CommitFifoState::Registered => {
                            has_unmet_dependencies = true;
                        }
                        CommitFifoState::Mailbox => {}
                    }
                }
                let commit_times = &mut *c.commit_times.borrow_mut();
                match commit_times {
                    CommitTimesState::Ready => {}
                    CommitTimesState::Queued { time } => {
                        *commit_times = register_commit_time(tl, self, c, *time)?;
                        if let CommitTimesState::Registered { .. } = commit_times {
                            has_unmet_dependencies = true;
                        }
                    }
                    CommitTimesState::Registered { .. } => {
                        has_unmet_dependencies = true;
                    }
                }
                if let Some(restored) = &c.toplevel_restored
                    && !restored.get()
                {
                    tl.toplevel_restored_waiter.set(Some(self.inner.clone()));
                    has_unmet_dependencies = true;
                }
                if !has_unmet_dependencies && let Some(serial) = c.serial {
                    c.surface.unblock_transactions_until(serial);
                    if c.surface.surface_transaction.commit_is_blocked(serial) {
                        tl.serial_waiter.set(Some(self.inner.clone()));
                        has_unmet_dependencies = true;
                    }
                }
                if has_unmet_dependencies {
                    return Ok(false);
                }
                c.surface.apply_state(c.pending.borrow_mut().deref_mut())?;
                Ok(true)
            }
            EntryKind::Wait(signaled) => Ok(signaled.get()),
            EntryKind::Signal(next, signaled) => {
                signaled.set(true);
                flush_list(next)?;
                Ok(true)
            }
            EntryKind::Gc(id) => {
                self.shared.gc.remove(id);
                Ok(true)
            }
        }
    }
}

fn check_shm_uploads(entry: &Rc<Entry>, c: &Commit) -> Result<(), WlSurfaceError> {
    let state = &mut *c.shm_upload.borrow_mut();
    if let ShmUploadState::Todo = state {
        let mut pending = SmallVec::new();
        schedule_async_uploads(entry, &c.surface, &c.pending.borrow(), &mut pending)?;
        c.pending_uploads.set(pending.len());
        *state = ShmUploadState::Scheduled(pending);
    }
    Ok(())
}

fn check_prime_copies(entry: &Rc<Entry>, c: &Commit) -> Result<(), WlSurfaceError> {
    let validities = &mut *c.prime_validities.borrow_mut();
    if let Some(validities) = validities
        && validities.iter().all(|v| v.valid())
    {
        return Ok(());
    }
    let validities = validities.get_or_insert_default();
    validities.clear();
    let mut copies = c.prime_copies.take().unwrap_or_default();
    copies.clear();
    schedule_prime_copies(
        entry,
        &c.surface,
        &mut c.pending.borrow_mut(),
        &mut copies,
        validities,
    )?;
    c.pending_prime_copies.set(copies.len());
    c.prime_copies.set(Some(copies));
    Ok(())
}

fn register_commit_time(
    tl: &CommitTimeline,
    rc: &Rc<Entry>,
    c: &Commit,
    time: u64,
) -> Result<CommitTimesState, WlSurfaceError> {
    let output = c.surface.output.get();
    let render_margin = output.render_margin_ns.get();
    let flip_margin = output.flip_margin_ns.get().unwrap_or_default();
    let refresh = output.global.refresh_nsec.get();
    let present_margin = render_margin.saturating_add(flip_margin).min(refresh);
    let timeout = time.saturating_sub(present_margin);
    if timeout <= c.surface.client.state.now_nsec() {
        return Ok(CommitTimesState::Ready);
    }
    let pending = tl
        .shared
        .ring
        .timeout_external(timeout, rc.clone(), 0)
        .map_err(WlSurfaceError::RegisterCommitTimeout)?;
    *tl.commit_time_waiter.borrow_mut() = Some(CommitTimeWaiter {
        node: rc.clone(),
        present: time,
    });
    c.surface
        .before_latch_listener
        .attach(&output.before_latch_event);
    Ok(CommitTimesState::Registered { _pending: pending })
}

fn schedule_async_uploads(
    node_ref: &Rc<Entry>,
    surface: &WlSurface,
    pending: &PendingState,
    uploads: &mut SmallVec<[PendingShmTransfer; 1]>,
) -> Result<(), WlSurfaceError> {
    if let Some(pending) = schedule_async_upload(node_ref, surface, pending)? {
        uploads.push(pending);
    }
    for ss in pending.subsurfaces.values() {
        if let Some(state) = &ss.pending.state {
            schedule_async_uploads(node_ref, &ss.subsurface.surface, state, uploads)?;
        }
    }
    Ok(())
}

fn schedule_async_upload(
    node_ref: &Rc<Entry>,
    surface: &WlSurface,
    pending: &PendingState,
) -> Result<Option<PendingShmTransfer>, WlSurfaceError> {
    let Some(Some(buf)) = &pending.buffer else {
        return Ok(None);
    };
    let buf = &buf.buf;
    let Some(WlBufferStorage::Shm {
        mem,
        stride,
        dmabuf_buffer_params,
    }) = &mut *buf.storage.borrow_mut()
    else {
        return Ok(None);
    };
    let back = surface.shm_textures.back();
    let state = &surface.client.state;
    let ctx = state
        .render_ctx
        .get()
        .ok_or(WlSurfaceError::NoRenderContext)?;
    if ctx.fast_ram_access()
        && buf
            .import_udmabuf_texture(&ctx, mem, *stride, dmabuf_buffer_params)
            .is_some()
    {
        back.damage.clear();
        back.tex.take();
        if surface.shm_textures.front().tex.is_none() {
            surface.shm_staging.take();
        }
        return Ok(None);
    }
    let mut back_tex_opt = back.tex.get();
    if let Some(back_tex) = &back_tex_opt
        && !back_tex.compatible_with(buf.format, buf.rect.width(), buf.rect.height(), *stride)
    {
        back_tex_opt = None;
    }
    let damage_full = || {
        back.damage.clear();
        back.damage.damage(slice::from_ref(&buf.rect));
    };
    let back_tex = match back_tex_opt {
        Some(b) => {
            if pending.damage_full || pending.surface_damage.is_not_empty() {
                damage_full();
            } else {
                back.damage.damage(&pending.buffer_damage);
            }
            b
        }
        None => {
            damage_full();
            let back_tex = ctx
                .clone()
                .async_shmem_texture(
                    buf.format,
                    buf.rect.width(),
                    buf.rect.height(),
                    *stride,
                    &state.cpu_worker,
                )
                .map_err(WlSurfaceError::CreateAsyncShmTexture)?;
            back.tex.set(Some(back_tex.clone()));
            back_tex
        }
    };
    if let Some(hb) = buf.get_gfx_buffer(&ctx, mem, dmabuf_buffer_params) {
        return back_tex
            .async_upload_from_buffer(&hb, node_ref.clone(), back.damage.get())
            .map_err(WlSurfaceError::PrepareAsyncUpload);
    }
    let mut staging_opt = surface.shm_staging.get();
    if let Some(staging) = &staging_opt
        && staging.size() != back_tex.staging_size()
    {
        staging_opt = None;
    }
    let staging = match staging_opt {
        Some(s) => s,
        None => {
            let s = surface
                .client
                .state
                .render_ctx
                .get()
                .ok_or(WlSurfaceError::NoRenderContext)?
                .create_staging_buffer(back_tex.staging_size(), STAGING_UPLOAD);
            surface.shm_staging.set(Some(s.clone()));
            s
        }
    };
    back_tex
        .async_upload(&staging, node_ref.clone(), mem.clone(), back.damage.get())
        .map_err(WlSurfaceError::PrepareAsyncUpload)
}

fn schedule_prime_copies(
    entry: &Rc<Entry>,
    surface: &WlSurface,
    pending: &mut PendingState,
    copies: &mut SmallVec<[PendingChainedCopy; 1]>,
    validities: &mut SmallVec<[PrimeValidity; 1]>,
) -> Result<(), WlSurfaceError> {
    let state = &surface.client.state;
    let Some(ctx) = state.render_ctx.get() else {
        return Ok(());
    };
    let dev = state.render_ctx_prime_copy_device.get();
    schedule_prime_copies_(
        &ctx,
        dev.as_ref(),
        entry,
        surface,
        pending,
        copies,
        validities,
    )
}

fn schedule_prime_copies_(
    ctx: &Rc<dyn GfxContext>,
    render_device: Option<&Rc<CopyDevice>>,
    entry: &Rc<Entry>,
    surface: &WlSurface,
    pending: &mut PendingState,
    copies: &mut SmallVec<[PendingChainedCopy; 1]>,
    validities: &mut SmallVec<[PrimeValidity; 1]>,
) -> Result<(), WlSurfaceError> {
    let (copy, validity) = schedule_prime_copy(&ctx, render_device, entry, surface, pending)?;
    if let Some(copy) = copy {
        copies.push(copy);
    }
    if let Some(validity) = validity {
        validities.push(validity);
    }
    for ss in pending.subsurfaces.values_mut() {
        if let Some(state) = &mut ss.pending.state {
            schedule_prime_copies_(
                &ctx,
                render_device,
                entry,
                &ss.subsurface.surface,
                state,
                copies,
                validities,
            )?;
        }
    }
    Ok(())
}

fn schedule_prime_copy(
    ctx: &Rc<dyn GfxContext>,
    render_dev: Option<&Rc<CopyDevice>>,
    entry: &Rc<Entry>,
    surface: &WlSurface,
    pending: &mut PendingState,
) -> Result<(Option<PendingChainedCopy>, Option<PrimeValidity>), WlSurfaceError> {
    let Some(Some(buf)) = &pending.buffer else {
        return Ok((None, None));
    };
    let buf = &buf.buf;
    if !buf.is_dmabuf() {
        return Ok((None, None));
    }
    let Some(WlBufferStorage::Dmabuf(storage)) = &mut *buf.storage.borrow_mut() else {
        return Ok((None, None));
    };
    let damage = if pending.damage_full || pending.surface_damage.is_not_empty() {
        slice::from_ref(&buf.rect)
    } else {
        &pending.buffer_damage
    };
    let validity = surface.prime.validity();
    let allow_lazy = ctx.supports_wait_sync();
    let copies = surface
        .prepare_prime_copies(ctx, render_dev, buf, storage, damage, allow_lazy)
        .map_err(WlSurfaceError::PreparePrimeCopy)?;
    let Some((copies, psb)) = copies else {
        pending.prime_buffer = None;
        return Ok((None, Some(validity)));
    };
    let chain = surface
        .client
        .state
        .schedule_chained_copy(&copies, entry.clone(), psb.take_sync(), Some(psb.damage()))
        .map_err(WlSurfaceError::PrimeCopy)?;
    pending.prime_buffer = Some(psb);
    Ok((chain, Some(validity)))
}

type Point = (Rc<Syncobj>, SyncobjPoint);

type RenderCtx = (Rc<dyn GfxContext>, Option<Rc<CopyDevice>>);

struct CommitDataCollector<F> {
    render_ctx: LazyCell<Option<RenderCtx>, F>,
    acquire_points: SmallVec<[Point; 1]>,
    shm_uploads: usize,
    has_dmabuf: bool,
    needs_prime_copies: bool,
    acquire_files: SmallVec<[Rc<OwnedFd>; 1]>,
    commit_time: u64,
    toplevel_restored: Option<Rc<Cell<bool>>>,
}

impl<F> CommitDataCollector<F>
where
    F: FnOnce() -> Option<RenderCtx>,
{
    fn collect(&mut self, pending: &mut PendingState) {
        if let Some(Some(buffer)) = &pending.buffer {
            let buffer = &buffer.buf;
            if buffer.is_shm() {
                self.shm_uploads += 1;
            }
            if buffer.is_dmabuf() {
                self.has_dmabuf = true;
                if let Some((ctx, cd)) = &*self.render_ctx
                    && buffer.needs_prime_copy(ctx, cd.as_ref())
                {
                    self.needs_prime_copies = true;
                }
            }
            if !pending.syncobj_sync
                && pending.sync_file_acquire.is_none()
                && let Some(dmabuf) = &buffer.client_dmabuf
            {
                for plane in &dmabuf.planes {
                    self.acquire_files.push(plane.fd.clone());
                }
            }
        }
        if let Some(Some(sf)) = pending.sync_file_acquire.take() {
            self.acquire_files.push(sf.0);
        }
        if let Some(point) = pending.acquire_point.take() {
            self.acquire_points.push(point);
        }
        if let Some(commit_time) = pending.commit_time.take() {
            self.commit_time = self.commit_time.max(commit_time);
        }
        if let Some(restored) = pending.xdg_surface.restored.take()
            && !restored.get()
        {
            self.toplevel_restored = Some(restored);
        }
        for ss in pending.subsurfaces.values_mut() {
            if let Some(state) = &mut ss.pending.state {
                self.collect(state);
            }
        }
    }
}

fn set_effective_timeline(
    timeline: &CommitTimeline,
    pending: &PendingState,
    effective: &Rc<Inner>,
) {
    if timeline.effective_timeline.id() != effective.id {
        let prev = timeline.effective_timeline.set(effective.clone());
        if prev.entries.is_not_empty() {
            let signaled = Rc::new(Cell::new(false));
            add_entry(
                effective,
                &timeline.shared,
                EntryKind::Wait(signaled.clone()),
            );
            add_entry(
                &prev,
                &timeline.shared,
                EntryKind::Signal(effective.clone(), signaled),
            );
        }
    }
    for ss in pending.subsurfaces.values() {
        if let Some(state) = &ss.pending.state {
            set_effective_timeline(&ss.subsurface.surface.commit_timeline, state, effective);
        }
    }
}

#[derive(Default)]
struct FlushRequests {
    flush_waiters: AsyncQueue<Rc<Inner>>,
}

async fn process_flush_requests(client: Weak<Client>, requests: Rc<FlushRequests>) {
    loop {
        requests.flush_waiters.non_empty().await;
        while let Some(entry) = requests.flush_waiters.try_pop() {
            if let Err(e) = flush_list(&entry) {
                if let Some(client) = client.upgrade() {
                    client.error(e);
                }
                return;
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum CommitFifoState {
    Queued,
    Registered,
    Mailbox,
}
