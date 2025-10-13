use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        client::Client,
        gfx_api::{AsyncShmGfxTextureCallback, GfxError, PendingShmTransfer, STAGING_UPLOAD},
        ifs::{
            wl_buffer::WlBufferStorage,
            wl_surface::{PendingState, WlSurface, WlSurfaceError},
        },
        io_uring::{
            IoUring, IoUringError, PendingPoll, PendingTimeout, PollCallback, TimeoutCallback,
        },
        tree::{BeforeLatchResult, TreeSerial},
        utils::{
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
            linkedlist::{LinkedList, LinkedNode, NodeRef},
            numcell::NumCell,
            oserror::OsError,
            queue::AsyncQueue,
        },
        video::drm::{
            DrmError,
            sync_obj::{SyncObj, SyncObjPoint},
            wait_for_sync_obj::{SyncObjWaiter, WaitForSyncObj, WaitForSyncObjHandle},
        },
    },
    isnt::std_1::{primitive::IsntSliceExt, vec::IsntVecExt},
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        mem,
        ops::{Deref, DerefMut},
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
    wfs: Rc<WaitForSyncObj>,
    ring: Rc<IoUring>,
    depth: NumCell<usize>,
    gc: CopyHashMap<CommitTimelineId, LinkedList<Entry>>,
    flush_requests: Rc<FlushRequests>,
    _flush_requests_future: SpawnedFuture<()>,
}

struct CommitTimeWaiter {
    node: NodeRef<Entry>,
    present: u64,
}

pub struct CommitTimeline {
    shared: Rc<CommitTimelines>,
    own_timeline: Rc<Inner>,
    effective_timeline: CloneCell<Rc<Inner>>,
    effective_timeline_id: Cell<CommitTimelineId>,
    fifo_barrier_set: Cell<bool>,
    fifo_waiter: Cell<Option<NodeRef<Entry>>>,
    commit_time_waiter: RefCell<Option<CommitTimeWaiter>>,
    tree_block_waiter: Cell<Option<NodeRef<Entry>>>,
}

struct Inner {
    id: CommitTimelineId,
    entries: LinkedList<Entry>,
}

fn add_entry(
    list: &LinkedList<Entry>,
    shared: &Rc<CommitTimelines>,
    kind: EntryKind,
) -> NodeRef<Entry> {
    shared.depth.fetch_add(1);
    let link = list.add_last(Entry {
        link: Cell::new(None),
        shared: shared.clone(),
        kind,
    });
    let noderef = link.to_ref();
    noderef.link.set(Some(link));
    noderef
}

#[derive(Debug, Error)]
pub enum CommitTimelineError {
    #[error(transparent)]
    ImmediateCommit(WlSurfaceError),
    #[error("Could not apply a delayed commit")]
    DelayedCommit(#[source] WlSurfaceError),
    #[error("Could not register a wait")]
    RegisterWait(#[source] DrmError),
    #[error("Syncobj wait failed")]
    Wait(#[source] DrmError),
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
}

impl CommitTimelines {
    pub fn new(
        wfs: &Rc<WaitForSyncObj>,
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
            effective_timeline: CloneCell::new(timeline),
            effective_timeline_id: Cell::new(id),
            fifo_barrier_set: Cell::new(false),
            fifo_waiter: Default::default(),
            commit_time_waiter: Default::default(),
            tree_block_waiter: Default::default(),
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

fn break_loops(list: &LinkedList<Entry>) {
    for entry in list.iter() {
        entry.link.take();
        if let EntryKind::Commit(c) = &entry.kind {
            c.wait_handles.take();
            *c.shm_upload.borrow_mut() = ShmUploadState::None;
            c.pending_polls.take();
        }
    }
}

impl CommitTimeline {
    pub fn clear(&self, reason: ClearReason) {
        match reason {
            ClearReason::BreakLoops => {
                self.fifo_waiter.take();
                self.commit_time_waiter.take();
                self.tree_block_waiter.take();
                break_loops(&self.own_timeline.entries)
            }
            ClearReason::Destroy => {
                self.clear_fifo_barrier();
                if self.own_timeline.entries.is_not_empty() {
                    let list = LinkedList::new();
                    list.append_all(&self.own_timeline.entries);
                    add_entry(&list, &self.shared, EntryKind::Gc(self.own_timeline.id));
                    self.shared.gc.set(self.own_timeline.id, list);
                }
            }
        }
    }

    pub(super) fn commit(
        &self,
        surface: &Rc<WlSurface>,
        pending: &mut Box<PendingState>,
    ) -> Result<(), CommitTimelineError> {
        let mut collector = CommitDataCollector {
            acquire_points: Default::default(),
            shm_uploads: 0,
            implicit_dmabufs: Default::default(),
            commit_time: Default::default(),
        };
        collector.collect(pending);
        let points = collector.acquire_points;
        let pending_uploads = collector.shm_uploads;
        let implicit_dmabufs = collector.implicit_dmabufs;
        let commit_time = collector.commit_time;
        let has_commit_time = commit_time > 0;
        let has_dependencies = points.is_not_empty()
            || pending_uploads > 0
            || implicit_dmabufs.is_not_empty()
            || has_commit_time;
        let mut must_be_queued = has_dependencies
            || self.own_timeline.entries.is_not_empty()
            || (pending.fifo_barrier_wait && self.fifo_barrier_set.get());
        if !must_be_queued && let Some(serial) = pending.serial {
            surface.handle_acked_serial(serial);
            must_be_queued = surface.serial_is_blocked(serial);
        }
        if !must_be_queued {
            return surface
                .apply_state(pending)
                .map_err(CommitTimelineError::ImmediateCommit);
        }
        if self.shared.depth.get() >= MAX_TIMELINE_DEPTH {
            return Err(CommitTimelineError::Depth);
        }
        set_effective_timeline(self, pending, &self.own_timeline);
        let commit_fifo_state = match pending.fifo_barrier_wait {
            true => CommitFifoState::Queued,
            false => CommitFifoState::Mailbox,
        };
        let has_serial = pending.serial.is_some();
        let noderef = add_entry(
            &self.own_timeline.entries,
            &self.shared,
            EntryKind::Commit(Commit {
                surface: surface.clone(),
                serial: pending.serial,
                pending: RefCell::new(mem::take(pending)),
                sync_obj: NumCell::new(points.len()),
                wait_handles: Cell::new(Default::default()),
                pending_uploads: NumCell::new(pending_uploads),
                shm_upload: RefCell::new(ShmUploadState::None),
                num_pending_polls: NumCell::new(implicit_dmabufs.len()),
                pending_polls: Cell::new(Default::default()),
                fifo_state: Cell::new(commit_fifo_state),
                commit_times: RefCell::new(CommitTimesState::Ready),
            }),
        );
        let mut needs_flush = commit_fifo_state == CommitFifoState::Queued || has_serial;
        if has_dependencies {
            let noderef = Rc::new(noderef.clone());
            let EntryKind::Commit(commit) = &noderef.kind else {
                unreachable!();
            };
            if points.is_not_empty() {
                let mut wait_handles = SmallVec::new();
                for (sync_obj, point) in points {
                    let handle = self
                        .shared
                        .wfs
                        .wait(&sync_obj, point, true, noderef.clone())
                        .map_err(CommitTimelineError::RegisterWait)?;
                    wait_handles.push(handle);
                }
                commit.wait_handles.set(wait_handles);
            }
            if pending_uploads > 0 {
                *commit.shm_upload.borrow_mut() = ShmUploadState::Todo(noderef.clone());
                needs_flush = true;
            }
            if implicit_dmabufs.is_not_empty() {
                let mut pending_polls = SmallVec::new();
                for fd in implicit_dmabufs {
                    let handle = self
                        .shared
                        .ring
                        .readable_external(&fd, noderef.clone())
                        .map_err(CommitTimelineError::RegisterImplicitPoll)?;
                    pending_polls.push(handle);
                }
                commit.pending_polls.set(pending_polls);
            }
            if has_commit_time {
                *commit.commit_times.borrow_mut() = CommitTimesState::Queued {
                    rc: noderef.clone(),
                    time: commit_time,
                };
                needs_flush = true;
            }
        }
        if needs_flush && noderef.prev().is_none() {
            flush_from(noderef.clone()).map_err(CommitTimelineError::DelayedCommit)?;
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
                    .push(w.node.clone());
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

    pub fn tree_unblocked(&self, surface: &WlSurface) {
        if let Some(waiter) = self.tree_block_waiter.take() {
            flush_surface(&waiter, surface);
        }
    }
}

impl SyncObjWaiter for NodeRef<Entry> {
    fn done(self: Rc<Self>, result: Result<(), DrmError>) {
        let EntryKind::Commit(commit) = &self.kind else {
            unreachable!();
        };
        if let Err(e) = result {
            commit.surface.client.error(CommitTimelineError::Wait(e));
            return;
        }
        commit.sync_obj.fetch_sub(1);
        flush_commit(&self, commit);
    }
}

fn flush_commit(node_ref: &NodeRef<Entry>, commit: &Commit) {
    flush_surface(node_ref, &commit.surface);
}

fn flush_surface(node_ref: &NodeRef<Entry>, surface: &WlSurface) {
    if let Err(e) = flush_from(node_ref.clone()) {
        surface.client.error(CommitTimelineError::DelayedCommit(e));
    }
}

impl AsyncShmGfxTextureCallback for NodeRef<Entry> {
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
        flush_commit(&self, commit);
    }
}

impl PollCallback for NodeRef<Entry> {
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
        flush_commit(&self, commit);
    }
}

impl TimeoutCallback for NodeRef<Entry> {
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
        flush_commit(&self, commit);
    }
}

struct Entry {
    link: Cell<Option<LinkedNode<Entry>>>,
    shared: Rc<CommitTimelines>,
    kind: EntryKind,
}

enum EntryKind {
    Commit(Commit),
    Wait(Cell<bool>),
    Signal(NodeRef<Entry>),
    Gc(CommitTimelineId),
}

enum ShmUploadState {
    None,
    Todo(Rc<NodeRef<Entry>>),
    Scheduled(#[expect(dead_code)] SmallVec<[PendingShmTransfer; 1]>),
}

enum CommitTimesState {
    Ready,
    Queued { rc: Rc<NodeRef<Entry>>, time: u64 },
    Registered { _pending: PendingTimeout },
}

struct Commit {
    surface: Rc<WlSurface>,
    pending: RefCell<Box<PendingState>>,
    serial: Option<TreeSerial>,
    sync_obj: NumCell<usize>,
    wait_handles: Cell<SmallVec<[WaitForSyncObjHandle; 1]>>,
    pending_uploads: NumCell<usize>,
    shm_upload: RefCell<ShmUploadState>,
    num_pending_polls: NumCell<usize>,
    pending_polls: Cell<SmallVec<[PendingPoll; 1]>>,
    fifo_state: Cell<CommitFifoState>,
    commit_times: RefCell<CommitTimesState>,
}

fn flush_from(mut point: NodeRef<Entry>) -> Result<(), WlSurfaceError> {
    let mut gc_list = None;
    while point.maybe_apply(&mut gc_list)? {
        point.shared.depth.fetch_sub(1);
        let _link = point.link.take();
        match point.next() {
            None => break,
            Some(n) => point = n,
        }
    }
    Ok(())
}

impl NodeRef<Entry> {
    fn maybe_apply(&self, gc_list: &mut Option<LinkedList<Entry>>) -> Result<bool, WlSurfaceError> {
        if self.prev().is_some() {
            return Ok(false);
        }
        match &self.kind {
            EntryKind::Commit(c) => {
                let mut has_unmet_dependencies = false;
                if c.sync_obj.get() > 0 {
                    has_unmet_dependencies = true;
                }
                if c.pending_uploads.get() > 0 {
                    check_shm_uploads(c)?;
                    if c.pending_uploads.get() > 0 {
                        has_unmet_dependencies = true;
                    }
                }
                if c.num_pending_polls.get() > 0 {
                    has_unmet_dependencies = true;
                }
                let tl = &c.surface.commit_timeline;
                if tl.fifo_barrier_set.get() {
                    match c.fifo_state.get() {
                        CommitFifoState::Queued => {
                            tl.fifo_waiter.set(Some(self.clone()));
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
                    CommitTimesState::Queued { rc, time } => {
                        *commit_times = register_commit_time(tl, rc, c, *time)?;
                        if let CommitTimesState::Registered { .. } = commit_times {
                            has_unmet_dependencies = true;
                        }
                    }
                    CommitTimesState::Registered { .. } => {
                        has_unmet_dependencies = true;
                    }
                }
                if has_unmet_dependencies {
                    return Ok(false);
                }
                if let Some(serial) = c.serial {
                    c.surface.handle_acked_serial(serial);
                    if c.surface.serial_is_blocked(serial) {
                        tl.tree_block_waiter.set(Some(self.clone()));
                        return Ok(false);
                    }
                }
                c.surface.apply_state(c.pending.borrow_mut().deref_mut())?;
                Ok(true)
            }
            EntryKind::Wait(signaled) => Ok(signaled.get()),
            EntryKind::Signal(s) => match &s.kind {
                EntryKind::Wait(signaled) => {
                    signaled.set(true);
                    flush_from(s.clone())?;
                    Ok(true)
                }
                _ => unreachable!(),
            },
            EntryKind::Gc(id) => {
                *gc_list = self.shared.gc.remove(id);
                Ok(true)
            }
        }
    }
}

fn check_shm_uploads(c: &Commit) -> Result<(), WlSurfaceError> {
    let state = &mut *c.shm_upload.borrow_mut();
    if let ShmUploadState::Todo(node_ref) = state {
        let mut pending = SmallVec::new();
        schedule_async_uploads(node_ref, &c.surface, &c.pending.borrow(), &mut pending)?;
        c.pending_uploads.set(pending.len());
        *state = ShmUploadState::Scheduled(pending);
    }
    Ok(())
}

fn register_commit_time(
    tl: &CommitTimeline,
    rc: &Rc<NodeRef<Entry>>,
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
        node: rc.deref().clone(),
        present: time,
    });
    c.surface
        .before_latch_listener
        .attach(&output.before_latch_event);
    Ok(CommitTimesState::Registered { _pending: pending })
}

fn schedule_async_uploads(
    node_ref: &Rc<NodeRef<Entry>>,
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
    node_ref: &Rc<NodeRef<Entry>>,
    surface: &WlSurface,
    pending: &PendingState,
) -> Result<Option<PendingShmTransfer>, WlSurfaceError> {
    let Some(Some(buf)) = &pending.buffer else {
        return Ok(None);
    };
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
    if ctx.fast_ram_access() && buf.import_udmabuf_texture(&ctx, mem, *stride, dmabuf_buffer_params)
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

type Point = (Rc<SyncObj>, SyncObjPoint);

struct CommitDataCollector {
    acquire_points: SmallVec<[Point; 1]>,
    shm_uploads: usize,
    implicit_dmabufs: SmallVec<[Rc<OwnedFd>; 1]>,
    commit_time: u64,
}

impl CommitDataCollector {
    fn collect(&mut self, pending: &mut PendingState) {
        if let Some(Some(buffer)) = &pending.buffer {
            if buffer.is_shm() {
                self.shm_uploads += 1;
            }
            if !pending.explicit_sync
                && let Some(dmabuf) = &buffer.dmabuf
            {
                for plane in &dmabuf.planes {
                    self.implicit_dmabufs.push(plane.fd.clone());
                }
            }
        }
        if let Some(point) = pending.acquire_point.take() {
            self.acquire_points.push(point);
        }
        if let Some(commit_time) = pending.commit_time.take() {
            self.commit_time = self.commit_time.max(commit_time);
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
    if timeline.effective_timeline_id.replace(effective.id) != effective.id {
        let prev = timeline.effective_timeline.set(effective.clone());
        if prev.entries.is_not_empty() {
            let noderef = add_entry(
                &effective.entries,
                &timeline.shared,
                EntryKind::Wait(Cell::new(false)),
            );
            add_entry(&prev.entries, &timeline.shared, EntryKind::Signal(noderef));
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
    flush_waiters: AsyncQueue<NodeRef<Entry>>,
}

async fn process_flush_requests(client: Weak<Client>, requests: Rc<FlushRequests>) {
    loop {
        requests.flush_waiters.non_empty().await;
        while let Some(entry) = requests.flush_waiters.try_pop() {
            if let Err(e) = flush_from(entry) {
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
