use {
    crate::{
        configurable::{ConfigureGroup, DEFAULT_TIMEOUT_NS},
        state::State,
        tree::TreeSerial,
        utils::{
            asyncevent::AsyncEvent,
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            errorfmt::ErrorFmt,
            numcell::NumCell,
            on_drop::OnDrop,
            ptr_ext::{MutPtrExt, PtrExt},
            queue::AsyncQueue,
            stack::Stack,
            syncqueue::SyncQueue,
        },
    },
    std::{
        cell::{Cell, UnsafeCell},
        mem::MaybeUninit,
        rc::Rc,
    },
};

pub struct TreeTransactions {
    timeout_ns: Cell<u64>,
    unused_blockers: SyncQueue<Rc<TreeBlockerInner>>,
    all_blockers: SyncQueue<Rc<TreeBlockerInner>>,
    unblock_queue: AsyncQueue<Rc<TreeBlockerInner>>,
    timeout_queue: AsyncQueue<TreeBlocker>,
    timeout_changed: AsyncEvent,
    cached_ops: CachedOps,
    timeline_ids: TreeTransactionTimelineIds,

    live_transactions: NumCell<usize>,
    configurer: UnsafeCell<MaybeUninit<ConfigureGroup>>,
    blocker: UnsafeCell<MaybeUninit<TreeBlocker>>,
    last_timeline: Cell<TreeTransactionTimelineId>,
}

linear_ids!(TreeTransactionTimelineIds, TreeTransactionTimelineId, u64);

pub struct TreeTransactionTimeline {
    id: TreeTransactionTimelineId,
    prev: CloneCell<Option<TreeBlocker>>,
}

pub trait TreeTransactionOp: Sized + 'static {
    fn unblocked(self, serial: TreeSerial, timeout: bool);
}

pub trait CachedTreeTransactionOp: TreeTransactionOp {
    #[expect(private_interfaces)]
    fn get_cache(tt: &CachedOps) -> &Rc<TreeTransactionOpCache<Self>>;
}

struct TreeTransactionOpCache<T> {
    ops: Stack<Box<TreeTransactionOpWrapper<T>>>,
}

struct TreeTransactionOpWrapper<T> {
    cache: Rc<TreeTransactionOpCache<T>>,
    data: Option<T>,
}

pub struct TreeTransaction<'a> {
    live_transactions: &'a NumCell<usize>,
    configurer: &'a ConfigureGroup,
    blocker: &'a TreeBlocker,
    last_timeline: &'a Cell<TreeTransactionTimelineId>,
}

#[derive(Clone)]
pub struct TreeBlocker {
    version: u64,
    inner: Rc<TreeBlockerInner>,
}

struct TreeBlockerInner {
    version: NumCell<u64>,
    closed: Cell<bool>,
    timed_out: Cell<bool>,
    pending_barriers: NumCell<u32>,
    serial: Cell<TreeSerial>,
    start_ns: Cell<u64>,
    ops: SyncQueue<Box<dyn TreeTransactionWrapperDyn>>,
    transactions: Rc<TreeTransactions>,
}

pub struct TreeBarrier {
    version: u64,
    weak: bool,
    start_time_ns: u64,
    blocker: Rc<TreeBlockerInner>,
}

struct BarrierDropper {
    _barrier: TreeBarrier,
}

impl TreeTransactions {
    pub fn clear(&self) {
        self.unused_blockers.clear();
        self.unblock_queue.clear();
        self.timeout_queue.clear();
        self.timeout_changed.clear();
        self.cached_ops.clear();
        while let Some(blocker) = self.all_blockers.pop() {
            blocker.ops.clear();
        }
    }

    #[expect(dead_code)]
    pub fn timeline(&self) -> TreeTransactionTimeline {
        TreeTransactionTimeline {
            id: self.timeline_ids.next(),
            prev: Default::default(),
        }
    }
}

impl TreeTransactionTimeline {
    pub fn and_then(&self, tt: &TreeTransaction) {
        if let Some(prev) = self.prev.set(Some(tt.blocker())) {
            prev.then_unblock(tt);
        }
    }
}

impl State {
    pub fn tree_transaction(&self) -> TreeTransaction<'_> {
        let tt = &self.tree_transactions;
        if tt.live_transactions.fetch_add(1) == 0 {
            let mut serial = self.tree_serials.next();
            if serial.raw() as u32 == 0 {
                serial = self.tree_serials.next();
            }
            let tbi = match tt.unused_blockers.pop() {
                Some(tbi) => tbi,
                None => {
                    let tbi = Rc::new(TreeBlockerInner {
                        version: Default::default(),
                        closed: Cell::new(false),
                        timed_out: Cell::new(false),
                        pending_barriers: Default::default(),
                        // pending_barrier_bts: Default::default(),
                        serial: Cell::new(serial),
                        start_ns: Cell::new(0),
                        ops: Default::default(),
                        transactions: tt.clone(),
                        // bt: Cell::new(None),
                        // barrier_ids: Default::default(),
                    });
                    tt.all_blockers.push(tbi.clone());
                    tbi
                }
            };
            tbi.closed.set(false);
            tbi.timed_out.set(false);
            tbi.pending_barriers.set(0);
            tbi.serial.set(serial);
            tbi.start_ns.set(self.now_nsec());
            // tt.bt.set(Some(Backtrace::new_unresolved()));
            assert!(tbi.ops.is_empty());
            let blocker = TreeBlocker {
                version: tbi.version.get(),
                inner: tbi,
            };
            let configure_group = self.configure_groups.group(serial);
            unsafe {
                tt.configurer.get().deref_mut().write(configure_group);
                tt.blocker.get().deref_mut().write(blocker);
            }
            tt.last_timeline.set(TreeTransactionTimelineId::from_raw(0));
        }
        unsafe {
            TreeTransaction {
                live_transactions: &tt.live_transactions,
                configurer: tt.configurer.get().deref().assume_init_ref(),
                blocker: tt.blocker.get().deref().assume_init_ref(),
                last_timeline: &tt.last_timeline,
            }
        }
    }
}

impl<T> Default for TreeTransactionOpCache<T> {
    fn default() -> Self {
        Self {
            ops: Default::default(),
        }
    }
}

trait TreeTransactionWrapperDyn {
    fn unblocked(self: Box<Self>, serial: TreeSerial, timeout: bool);
}

impl<T> TreeTransactionOpCache<T> {
    fn get(self: &Rc<Self>, data: T) -> Box<TreeTransactionOpWrapper<T>> {
        let mut w = self.ops.pop().unwrap_or_else(|| {
            Box::new(TreeTransactionOpWrapper {
                cache: self.clone(),
                data: None,
            })
        });
        w.data = Some(data);
        w
    }

    fn clear(&self) {
        self.ops.take();
    }
}

impl<T> TreeTransactionWrapperDyn for TreeTransactionOpWrapper<T>
where
    T: TreeTransactionOp,
{
    fn unblocked(mut self: Box<Self>, serial: TreeSerial, timeout: bool) {
        if let Some(data) = self.data.take() {
            data.unblocked(serial, timeout);
        }
        let cache = self.cache.clone();
        cache.ops.push(self);
    }
}

unsafe impl UnsafeCellCloneSafe for TreeBlocker {}

impl Default for TreeTransactions {
    fn default() -> Self {
        Self {
            timeout_ns: Cell::new(DEFAULT_TIMEOUT_NS),
            unused_blockers: Default::default(),
            all_blockers: Default::default(),
            unblock_queue: Default::default(),
            timeout_queue: Default::default(),
            timeout_changed: Default::default(),
            cached_ops: Default::default(),
            timeline_ids: Default::default(),
            live_transactions: Default::default(),
            configurer: UnsafeCell::new(MaybeUninit::uninit()),
            blocker: UnsafeCell::new(MaybeUninit::uninit()),
            last_timeline: Cell::new(TreeTransactionTimelineId::from_raw(0)),
        }
    }
}

impl Drop for TreeBarrier {
    fn drop(&mut self) {
        let b = &self.blocker;
        if self.weak || self.version != b.version.get() {
            return;
        }
        if b.pending_barriers.fetch_sub(1) != 1 {
            return;
        }
        if !b.closed.get() {
            return;
        }
        b.unblock(false);
    }
}

impl TreeBarrier {
    pub fn is_blocked(&self) -> bool {
        self.version == self.blocker.version.get()
    }

    #[expect(dead_code)]
    pub fn is_unblocked(&self) -> bool {
        !self.is_blocked()
    }

    #[expect(dead_code)]
    pub fn timed_out(&self, now_ns: u64) -> bool {
        self.start_time_ns + self.blocker.transactions.timeout_ns.get() <= now_ns
    }
}

impl TreeBlockerInner {
    fn unblock(self: &Rc<Self>, timed_out: bool) {
        self.version.fetch_add(1);
        self.timed_out.set(timed_out);
        self.transactions.unblock_queue.push(self.clone());
    }
}

impl TreeTransactionOp for BarrierDropper {
    fn unblocked(self, _serial: TreeSerial, _timeout: bool) {
        // nothing
    }
}

impl TreeBlocker {
    fn add_op<T>(&self, op: T)
    where
        T: CachedTreeTransactionOp,
    {
        let i = &*self.inner;
        let op = T::get_cache(&i.transactions.cached_ops).get(op);
        i.ops.push(op);
    }

    pub fn then_unblock(&self, tt: &TreeTransaction) {
        if self.is_blocked() && self.inner.serial.get() < tt.serial() {
            self.add_op(BarrierDropper {
                _barrier: tt.barrier(),
            });
        }
    }

    pub fn is_blocked(&self) -> bool {
        self.version == self.inner.version.get()
    }

    #[expect(dead_code)]
    pub fn is_unblocked(&self) -> bool {
        !self.is_blocked()
    }
}

impl TreeTransaction<'_> {
    #[expect(dead_code)]
    pub fn add_op<T>(&self, timeline: &TreeTransactionTimeline, op: T)
    where
        T: CachedTreeTransactionOp,
    {
        if self.last_timeline.replace(timeline.id) != timeline.id {
            timeline.and_then(self);
        }
        self.blocker.add_op(op);
    }

    pub fn configure_group(&self) -> &ConfigureGroup {
        self.configurer
    }

    pub fn blocker(&self) -> TreeBlocker {
        self.blocker.clone()
    }

    fn barrier_(&self, weak: bool) -> TreeBarrier {
        TreeBarrier {
            version: self.blocker.version,
            weak,
            start_time_ns: self.blocker.inner.start_ns.get(),
            blocker: self.blocker.inner.clone(),
        }
    }

    pub fn barrier(&self) -> TreeBarrier {
        self.blocker.inner.pending_barriers.fetch_add(1);
        self.barrier_(false)
    }

    #[expect(dead_code)]
    pub fn weak_barrier(&self) -> TreeBarrier {
        self.barrier_(true)
    }

    pub fn serial(&self) -> TreeSerial {
        self.blocker.inner.serial.get()
    }
}

impl Drop for TreeTransaction<'_> {
    fn drop(&mut self) {
        if self.live_transactions.sub_fetch(1) > 0 {
            return;
        }
        let o = self.blocker;
        let b = &o.inner;
        b.closed.set(true);
        if b.pending_barriers.get() == 0 {
            if b.ops.is_empty() {
                b.version.fetch_add(1);
                b.transactions.unused_blockers.push(b.clone());
            } else {
                b.unblock(false);
            }
        } else if b.transactions.timeout_ns.get() == 0 {
            b.unblock(false);
        } else {
            b.transactions.timeout_queue.push(o.clone());
        }
        let tt = b.transactions.clone();
        unsafe {
            tt.configurer.get().deref_mut().assume_init_drop();
            tt.blocker.get().deref_mut().assume_init_drop();
        }
    }
}

pub async fn handle_tree_blocker_unblocked(state: Rc<State>) {
    let tb = &state.tree_transactions;
    loop {
        tb.unblock_queue.non_empty().await;
        while let Some(blocker) = tb.unblock_queue.try_pop() {
            let timed_out = blocker.timed_out.get();
            let serial = blocker.serial.get();
            while let Some(op) = blocker.ops.pop() {
                op.unblocked(serial, timed_out);
            }
            tb.unused_blockers.push(blocker);
        }
    }
}

pub async fn handle_tree_blocker_timeout(state: Rc<State>) {
    loop {
        let state2 = state.clone();
        let _timeout = state.eng.spawn("tree blocker timeout impl", async move {
            let timeout_ns = state2.tree_transactions.timeout_ns.get();
            loop {
                let blocker = state2.tree_transactions.timeout_queue.pop().await;
                if blocker.version != blocker.inner.version.get() {
                    continue;
                }
                let timeout_ns = blocker.inner.start_ns.get() + timeout_ns;
                if timeout_ns > state2.now_nsec() {
                    let push_back = OnDrop(|| {
                        state2
                            .tree_transactions
                            .timeout_queue
                            .push_front(blocker.clone())
                    });
                    let res = state2.ring.timeout(timeout_ns).await;
                    push_back.forget();
                    if let Err(e) = res {
                        log::error!("Could not wait for blocker timeout: {}", ErrorFmt(e));
                    }
                }
                if blocker.version == blocker.inner.version.get() {
                    blocker.inner.unblock(true);
                }
            }
        });
        state.tree_transactions.timeout_changed.triggered().await;
    }
}

macro_rules! ops {
    ($($t:ident,)*) => {
        #[derive(Default)]
        #[expect(non_snake_case)]
        struct CachedOps {
            $($t: Rc<TreeTransactionOpCache<$t>>,)*
        }

        impl CachedOps {
            fn clear(&self) {
                $(self.$t.clear();)*
            }
        }

        $(
            #[allow(private_interfaces)]
            impl CachedTreeTransactionOp for $t {
                fn get_cache(tt: &CachedOps) -> &Rc<TreeTransactionOpCache<Self>> {
                    &tt.$t
                }
            }
        )*
    };
}

ops! {
    BarrierDropper,
}
