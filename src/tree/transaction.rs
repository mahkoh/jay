use {
    crate::{
        configurable::ConfigureGroup,
        ifs::wl_surface::{
            WlSurfaceSetMaxUnblocked,
            ext_session_lock_surface_v1::ExtSessionLockSurfaceV1SetPosition,
            xdg_surface::xdg_toplevel::XdgToplevelTreeOp,
        },
        state::State,
        tree::{ContainerTreeOp, FloatChange, TreeSerial, WorkspaceTreeOp},
        utils::{
            asyncevent::AsyncEvent,
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            errorfmt::ErrorFmt,
            numcell::NumCell,
            on_drop::OnDrop,
            queue::AsyncQueue,
            stack::Stack,
            syncqueue::SyncQueue,
        },
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TreeTransactions {
    timeout_ns: Cell<u64>,
    unused_blockers: SyncQueue<Rc<TreeBlockerInner>>,
    all_blockers: SyncQueue<Rc<TreeBlockerInner>>,
    unblock_queue: AsyncQueue<Rc<TreeBlockerInner>>,
    timeout_queue: AsyncQueue<TreeBlocker>,
    timeout_changed: AsyncEvent,
    cached_ops: CachedOps,
}

#[derive(Default)]
pub struct TreeTransactionTimeline {
    prev: CloneCell<Option<TreeBlocker>>,
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
}

impl TreeTransactionTimeline {
    pub fn and_then(&self, tt: &TreeTransaction) {
        if let Some(prev) = self.prev.set(Some(tt.blocker())) {
            prev.then_unblock(tt);
        }
    }
}

impl State {
    pub fn tree_transaction(&self) -> TreeTransaction {
        let mut serial = self.tree_serials.next();
        if serial.raw() as u32 == 0 {
            serial = self.tree_serials.next();
        }
        let tt = match self.tree_transactions.unused_blockers.pop() {
            Some(tt) => tt,
            None => {
                let tt = Rc::new(TreeBlockerInner {
                    version: Default::default(),
                    version2: Cell::new(0),
                    closed: Cell::new(false),
                    timed_out: Cell::new(false),
                    pending_barriers: Default::default(),
                    serial: Cell::new(serial),
                    start_ns: Cell::new(0),
                    ops: Default::default(),
                    transactions: self.tree_transactions.clone(),
                });
                self.tree_transactions.all_blockers.push(tt.clone());
                tt
            }
        };
        tt.closed.set(false);
        tt.timed_out.set(false);
        tt.pending_barriers.set(0);
        tt.serial.set(serial);
        tt.start_ns.set(self.now_nsec());
        assert!(tt.ops.is_empty());
        let blocker = TreeBlocker {
            version: tt.version.get(),
            inner: tt,
        };
        TreeTransaction {
            configurer: self.configure_groups.group(blocker.inner.serial.get()),
            blocker,
        }
    }
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

pub struct TreeTransaction {
    configurer: ConfigureGroup,
    blocker: TreeBlocker,
}

#[derive(Clone)]
pub struct TreeBlocker {
    version: u64,
    inner: Rc<TreeBlockerInner>,
}

unsafe impl UnsafeCellCloneSafe for TreeBlocker {}

struct TreeBlockerInner {
    version: NumCell<u64>,
    version2: Cell<u64>,
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
    blocker: Rc<TreeBlockerInner>,
}

impl Default for TreeTransactions {
    fn default() -> Self {
        Self {
            timeout_ns: Cell::new(5_000_000_000),
            unused_blockers: Default::default(),
            all_blockers: Default::default(),
            unblock_queue: Default::default(),
            timeout_queue: Default::default(),
            timeout_changed: Default::default(),
            cached_ops: Default::default(),
        }
    }
}

impl Drop for TreeBarrier {
    fn drop(&mut self) {
        let b = &self.blocker;
        if self.version != b.version.get() {
            return;
        }
        // log::info!("unblock {:?}", b.serial.get());
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
    pub fn serial(&self) -> TreeSerial {
        self.blocker.serial.get()
    }

    pub fn is_blocked(&self) -> bool {
        self.version == self.blocker.version.get()
    }

    pub fn is_unblocked(&self) -> bool {
        !self.is_blocked()
    }
}

impl TreeBlockerInner {
    fn unblock(self: &Rc<Self>, timed_out: bool) {
        self.version.fetch_add(1);
        if timed_out {
            log::warn!("timeout!!");
        }
        self.timed_out.set(timed_out);
        self.transactions.unblock_queue.push(self.clone());
    }
}

struct BarrierDropper {
    _barrier: TreeBarrier,
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

impl TreeTransaction {
    pub fn add_op<T>(&self, op: T)
    where
        T: CachedTreeTransactionOp,
    {
        self.blocker.add_op(op);
    }

    pub fn configure_group(&self) -> &ConfigureGroup {
        &self.configurer
    }

    pub fn blocker(&self) -> TreeBlocker {
        self.blocker.clone()
    }

    pub fn barrier(&self) -> TreeBarrier {
        self.blocker.inner.pending_barriers.fetch_add(1);
        TreeBarrier {
            version: self.blocker.version,
            blocker: self.blocker.inner.clone(),
        }
    }

    pub fn serial(&self) -> TreeSerial {
        self.blocker.inner.serial.get()
    }
}

impl Drop for TreeTransaction {
    fn drop(&mut self) {
        let o = &self.blocker;
        let b = &o.inner;
        b.closed.set(true);
        if b.pending_barriers.get() == 0 {
            if b.ops.is_empty() {
                b.version.fetch_add(1);
                b.version2.set(b.version.get());
                b.transactions.unused_blockers.push(b.clone());
            } else {
                b.unblock(false);
            }
        } else if b.transactions.timeout_ns.get() == 0 {
            b.unblock(false);
        } else {
            // log::info!(
            //     "{:?} - {} barriers",
            //     b.serial.get(),
            //     b.pending_barriers.get()
            // );
            b.transactions.timeout_queue.push(o.clone());
        }
    }
}

// thread_local! {
//     pub static APPLYING: Cell<TreeSerial> = const { Cell::new(TreeSerial(0)) };
// }

pub async fn handle_tree_blocker_unblocked(state: Rc<State>) {
    let tb = &state.tree_transactions;
    loop {
        // log::info!("WAIT");
        tb.unblock_queue.non_empty().await;
        // log::info!("RUN");
        while let Some(blocker) = tb.unblock_queue.try_pop() {
            // let blocker = tb.unblock_queue.pop().await;
            let timed_out = blocker.timed_out.get();
            let serial = blocker.serial.get();
            // log::info!("start {:?}", serial);
            // APPLYING.set(serial);
            while let Some(op) = blocker.ops.pop() {
                op.unblocked(serial, timed_out);
            }
            // APPLYING.set(TreeSerial(0));
            // log::info!("stop");
            blocker.version2.set(blocker.version.get());
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
    WlSurfaceSetMaxUnblocked,
    FloatChange,
    ExtSessionLockSurfaceV1SetPosition,
    WorkspaceTreeOp,
    XdgToplevelTreeOp,
    ContainerTreeOp,
}
