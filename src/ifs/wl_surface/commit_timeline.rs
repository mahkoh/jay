use {
    crate::{
        gfx_api::{AsyncShmGfxTextureCallback, GfxError, PendingShmUpload, STAGING_UPLOAD},
        ifs::{
            wl_buffer::WlBufferStorage,
            wl_surface::{PendingState, WlSurface, WlSurfaceError},
        },
        io_uring::{IoUring, IoUringError, PendingPoll, PollCallback},
        utils::{
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
            linkedlist::{LinkedList, LinkedNode, NodeRef},
            numcell::NumCell,
            oserror::OsError,
        },
        video::drm::{
            sync_obj::{SyncObj, SyncObjPoint},
            wait_for_sync_obj::{SyncObjWaiter, WaitForSyncObj, WaitForSyncObjHandle},
            DrmError,
        },
    },
    isnt::std_1::{primitive::IsntSliceExt, vec::IsntVecExt},
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        mem,
        ops::DerefMut,
        rc::Rc,
        slice,
    },
    thiserror::Error,
    uapi::{c::c_short, OwnedFd},
};

const MAX_TIMELINE_DEPTH: usize = 256;

linear_ids!(CommitTimelineIds, CommitTimelineId, u64);

pub struct CommitTimelines {
    next_id: CommitTimelineIds,
    wfs: Rc<WaitForSyncObj>,
    ring: Rc<IoUring>,
    depth: NumCell<usize>,
    gc: CopyHashMap<CommitTimelineId, LinkedList<Entry>>,
}

pub struct CommitTimeline {
    shared: Rc<CommitTimelines>,
    own_timeline: Rc<Inner>,
    effective_timeline: CloneCell<Rc<Inner>>,
    effective_timeline_id: Cell<CommitTimelineId>,
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
}

impl CommitTimelines {
    pub fn new(wfs: &Rc<WaitForSyncObj>, ring: &Rc<IoUring>) -> Self {
        Self {
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
        }
    }

    pub fn clear(&self) {
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
            ClearReason::BreakLoops => break_loops(&self.own_timeline.entries),
            ClearReason::Destroy => {
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
        let mut points = SmallVec::new();
        let mut pending_uploads = 0;
        let mut implicit_dmabufs = SmallVec::new();
        collect_commit_data(
            pending,
            &mut points,
            &mut pending_uploads,
            &mut implicit_dmabufs,
        );
        let has_dependencies =
            points.is_not_empty() || pending_uploads > 0 || implicit_dmabufs.is_not_empty();
        if !has_dependencies && self.own_timeline.entries.is_empty() {
            return surface
                .apply_state(pending)
                .map_err(CommitTimelineError::ImmediateCommit);
        }
        if self.shared.depth.get() >= MAX_TIMELINE_DEPTH {
            return Err(CommitTimelineError::Depth);
        }
        set_effective_timeline(self, pending, &self.own_timeline);
        let noderef = add_entry(
            &self.own_timeline.entries,
            &self.shared,
            EntryKind::Commit(Commit {
                surface: surface.clone(),
                pending: RefCell::new(mem::take(pending)),
                sync_obj: NumCell::new(points.len()),
                wait_handles: Cell::new(Default::default()),
                pending_uploads: NumCell::new(pending_uploads),
                shm_upload: RefCell::new(ShmUploadState::None),
                num_pending_polls: NumCell::new(implicit_dmabufs.len()),
                pending_polls: Cell::new(Default::default()),
            }),
        );
        let mut needs_flush = false;
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
        }
        if needs_flush && noderef.prev().is_none() {
            flush_from(noderef.clone()).map_err(CommitTimelineError::DelayedCommit)?;
        }
        Ok(())
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
    if let Err(e) = flush_from(node_ref.clone()) {
        commit
            .surface
            .client
            .error(CommitTimelineError::DelayedCommit(e));
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
    Scheduled(#[expect(dead_code)] SmallVec<[PendingShmUpload; 1]>),
}

struct Commit {
    surface: Rc<WlSurface>,
    pending: RefCell<Box<PendingState>>,
    sync_obj: NumCell<usize>,
    wait_handles: Cell<SmallVec<[WaitForSyncObjHandle; 1]>>,
    pending_uploads: NumCell<usize>,
    shm_upload: RefCell<ShmUploadState>,
    num_pending_polls: NumCell<usize>,
    pending_polls: Cell<SmallVec<[PendingPoll; 1]>>,
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
                if has_unmet_dependencies {
                    return Ok(false);
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

fn schedule_async_uploads(
    node_ref: &Rc<NodeRef<Entry>>,
    surface: &WlSurface,
    pending: &PendingState,
    uploads: &mut SmallVec<[PendingShmUpload; 1]>,
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
) -> Result<Option<PendingShmUpload>, WlSurfaceError> {
    let Some(Some(buf)) = &pending.buffer else {
        return Ok(None);
    };
    let Some(WlBufferStorage::Shm { mem, stride, .. }) = &*buf.storage.borrow() else {
        return Ok(None);
    };
    let back = surface.shm_textures.back();
    let mut back_tex_opt = back.tex.get();
    if let Some(back_tex) = &back_tex_opt {
        if !back_tex.compatible_with(buf.format, buf.rect.width(), buf.rect.height(), *stride) {
            back_tex_opt = None;
        }
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
            let state = &surface.client.state;
            let ctx = state
                .render_ctx
                .get()
                .ok_or(WlSurfaceError::NoRenderContext)?;
            let back_tex = ctx
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
    let mut staging_opt = surface.shm_staging.get();
    if let Some(staging) = &staging_opt {
        if staging.size() != back_tex.staging_size() {
            staging_opt = None;
        }
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

fn collect_commit_data(
    pending: &mut PendingState,
    acquire_points: &mut SmallVec<[Point; 1]>,
    shm_uploads: &mut usize,
    implicit_dmabufs: &mut SmallVec<[Rc<OwnedFd>; 1]>,
) {
    if let Some(Some(buffer)) = &pending.buffer {
        if buffer.is_shm() {
            *shm_uploads += 1;
        }
        if !pending.explicit_sync {
            if let Some(dmabuf) = &buffer.dmabuf {
                for plane in &dmabuf.planes {
                    implicit_dmabufs.push(plane.fd.clone());
                }
            }
        }
    }
    if let Some(point) = pending.acquire_point.take() {
        acquire_points.push(point);
    }
    for ss in pending.subsurfaces.values_mut() {
        if let Some(state) = &mut ss.pending.state {
            collect_commit_data(state, acquire_points, shm_uploads, implicit_dmabufs);
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
