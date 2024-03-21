use {
    crate::{
        ifs::wl_surface::{PendingState, WlSurface, WlSurfaceError},
        utils::{
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            linkedlist::{LinkedList, LinkedNode, NodeRef},
            numcell::NumCell,
        },
        video::drm::{
            sync_obj::{SyncObj, SyncObjPoint},
            wait_for_sync_obj::{SyncObjWaiter, WaitForSyncObj, WaitForSyncObjHandle},
            DrmError,
        },
    },
    isnt::std_1::primitive::IsntSliceExt,
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        mem,
        ops::{Deref, DerefMut},
        rc::Rc,
    },
    thiserror::Error,
};

const MAX_TIMELINE_DEPTH: usize = 256;

linear_ids!(CommitTimelineIds, CommitTimelineId, u64);

pub struct CommitTimelines {
    next_id: CommitTimelineIds,
    wfs: Rc<WaitForSyncObj>,
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
}

impl CommitTimelines {
    pub fn new(wfs: &Rc<WaitForSyncObj>) -> Self {
        Self {
            next_id: Default::default(),
            depth: NumCell::new(0),
            wfs: wfs.clone(),
            gc: Default::default(),
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
        for (_, list) in self.gc.lock().drain() {
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
        consume_acquire_points(pending, &mut points);
        if points.is_empty() && self.own_timeline.entries.is_empty() {
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
            }),
        );
        if points.is_not_empty() {
            let mut wait_handles = SmallVec::new();
            let noderef = Rc::new(noderef);
            for (sync_obj, point) in points {
                let handle = self
                    .shared
                    .wfs
                    .wait(&sync_obj, point, true, noderef.clone())
                    .map_err(CommitTimelineError::RegisterWait)?;
                wait_handles.push(handle);
            }
            let EntryKind::Commit(commit) = &noderef.kind else {
                unreachable!();
            };
            commit.wait_handles.set(wait_handles);
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
        if let Err(e) = flush_from(self.deref().clone()) {
            commit
                .surface
                .client
                .error(CommitTimelineError::DelayedCommit(e));
        }
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

struct Commit {
    surface: Rc<WlSurface>,
    pending: RefCell<Box<PendingState>>,
    sync_obj: NumCell<usize>,
    wait_handles: Cell<SmallVec<[WaitForSyncObjHandle; 1]>>,
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
                if c.sync_obj.get() > 0 {
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

type Point = (Rc<SyncObj>, SyncObjPoint);

fn consume_acquire_points(pending: &mut PendingState, points: &mut SmallVec<[Point; 1]>) {
    if let Some(point) = pending.acquire_point.take() {
        points.push(point);
    }
    for ss in pending.subsurfaces.values_mut() {
        consume_acquire_points(&mut ss.state, points);
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
        set_effective_timeline(&ss.subsurface.surface.commit_timeline, &ss.state, effective);
    }
}
