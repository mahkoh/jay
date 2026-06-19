use {
    crate::{
        control_center::CCI_COMPOSITOR,
        ifs::wl_surface::{WlSurface, WlSurfaceTransactionOp},
        state::{State, StateTransactionOp, TreeState},
        tree::TreeSerial,
        utils::{
            asyncevent::AsyncEvent, errorfmt::ErrorFmt, numcell::NumCell, queue::AsyncQueue,
            stack::Stack, syncqueue::SyncQueue,
        },
    },
    derivative::Derivative,
    run_on_drop::on_drop,
    std::{cell::Cell, rc::Rc},
};

pub trait Transactionable: 'static {
    type T: Sized;

    fn data(&self) -> &TransactionData<Self::T>;

    fn apply(self: &Rc<Self>, op: Self::T);

    fn committed(&self) {
        // nothing
    }

    fn before_apply(self: &Rc<Self>) {
        // nothing
    }

    fn after_apply(self: &Rc<Self>) {
        // nothing
    }
}

trait TransactionableDyn {
    fn commit(&self, transaction: &Rc<Transaction>, blocker: &Rc<Blocker>);
    fn apply_dyn(self: Rc<Self>);
}

pub trait TransactionableExt: Transactionable {
    fn add_transaction_op(self: &Rc<Self>, op: Self::T);
    fn add_transaction_no_op(self: &Rc<Self>);
}

pub struct TransactionData<T> {
    state: Rc<TreeState>,
    scheduled: Cell<bool>,
    ops: SyncQueue<T>,
    committed: NumCell<usize>,
    transactions: SyncQueue<TransactionableTransaction>,
}

struct TransactionableTransaction {
    transaction: Rc<Transaction>,
    ops: usize,
}

const DEFAULT_TIMEOUT_NS: u64 = 0;

#[derive(Derivative)]
#[derivative(Default)]
pub struct Transactions {
    surfaces: Stack<Rc<WlSurface>>,
    scheduled: Stack<Rc<dyn TransactionableDyn>>,
    unused_transactions: Stack<Rc<Transaction>>,
    all_transactions: Stack<Rc<Transaction>>,
    apply: AsyncQueue<Rc<Transaction>>,
    timeout: AsyncQueue<TransactionTimeout>,
    #[derivative(Default(value = "Cell::new(DEFAULT_TIMEOUT_NS)"))]
    timeout_ns: Cell<u64>,
    timeout_changed: AsyncEvent,
}

#[derive(Default)]
pub struct TransactionsWork {
    scheduled: Vec<Rc<dyn TransactionableDyn>>,
    surfaces: Vec<Rc<WlSurface>>,
}

struct Blocker {
    version: u64,
    transaction: Rc<Transaction>,
}

struct Transaction {
    version: NumCell<u64>,
    state: Rc<State>,
    members: Stack<Rc<dyn TransactionableDyn>>,
    blockers: Stack<Rc<Blocker>>,
}

#[derive(Derivative)]
#[derivative(Default)]
pub struct SurfaceTransaction {
    first_transaction_blocker: Cell<Option<TreeSerial>>,
    transaction_blockers: SyncQueue<(TreeSerial, Rc<Blocker>)>,

    #[derivative(Default(value = "Cell::new(TreeSerial::NONE)"))]
    unblocked_commit: Cell<TreeSerial>,
    unblocked_commit_start_ns: Cell<u64>,

    tardy: Cell<bool>,
}

#[derive(Clone)]
struct TransactionTimeout {
    t: Rc<Transaction>,
    version: u64,
    start_ns: u64,
}

const LOG_TARDY: bool = false;

impl<T> TransactionData<T> {
    pub fn new(state: &Rc<TreeState>) -> Self {
        Self {
            state: state.clone(),
            scheduled: Default::default(),
            ops: Default::default(),
            committed: Default::default(),
            transactions: Default::default(),
        }
    }
}

impl<T> TransactionableDyn for T
where
    T: Transactionable,
{
    fn commit(&self, transaction: &Rc<Transaction>, blocker: &Rc<Blocker>) {
        let d = self.data();
        d.scheduled.set(false);
        let total = d.ops.len();
        let ops = total - d.committed.get();
        d.committed.set(total);
        if let Some(last) = d.transactions.pop_back() {
            last.transaction.blockers.push(blocker.clone());
            d.transactions.push(last);
        }
        d.transactions.push(TransactionableTransaction {
            transaction: transaction.clone(),
            ops,
        });
        self.committed();
    }

    fn apply_dyn(self: Rc<Self>) {
        let d = self.data();
        let Some(t) = d.transactions.pop() else {
            return;
        };
        self.before_apply();
        for _ in 0..t.ops {
            if let Some(op) = d.ops.pop() {
                self.apply(op);
            }
        }
        self.after_apply();
        d.committed.fetch_sub(t.ops);
    }
}

impl<T> TransactionableExt for T
where
    T: Transactionable,
{
    fn add_transaction_op(self: &Rc<Self>, op: Self::T) {
        let d = self.data();
        d.ops.push(op);
        self.add_transaction_no_op();
    }

    fn add_transaction_no_op(self: &Rc<Self>) {
        let d = self.data();
        if d.scheduled.replace(true) {
            return;
        }
        d.state.transactions.scheduled.push(self.clone());
        d.state.serial_groups.trigger();
    }
}

impl Transactions {
    pub fn add_surface(&self, surface: &Rc<WlSurface>) {
        self.surfaces.push(surface.clone());
    }

    pub fn commit(&self, state: &Rc<State>, work: &mut TransactionsWork, serial: TreeSerial) {
        let scheduled = &mut work.scheduled;
        self.scheduled.swap(scheduled);
        let surfaces = &mut work.surfaces;
        self.surfaces.swap(surfaces);
        let now_ns = state.now_nsec();
        for surface in &*surfaces {
            let d = Transactionable::data(&**surface);
            d.ops
                .push(WlSurfaceTransactionOp::UnblockCommitsUntil(serial, now_ns));
            if !d.scheduled.replace(true) {
                scheduled.push(surface.clone());
            }
        }
        if scheduled.is_empty() {
            assert!(surfaces.is_empty());
            return;
        }
        let t = match self.unused_transactions.pop() {
            Some(t) => t,
            _ => {
                let t = Rc::new(Transaction {
                    version: Default::default(),
                    state: state.clone(),
                    members: Default::default(),
                    blockers: Default::default(),
                });
                self.all_transactions.push(t.clone());
                t
            }
        };
        let blocker = Rc::new(Blocker {
            version: t.version.get(),
            transaction: t.clone(),
        });
        while let Some(surface) = surfaces.pop() {
            if !surface.surface_transaction.tardy.get() {
                let st = &surface.surface_transaction;
                if st.transaction_blockers.is_empty() {
                    st.first_transaction_blocker.set(Some(serial));
                }
                st.transaction_blockers.push((serial, blocker.clone()));
            }
        }
        while let Some(transactionable) = scheduled.pop() {
            transactionable.commit(&t, &blocker);
            t.members.push(transactionable);
        }
        self.timeout.push(TransactionTimeout {
            version: t.version.get(),
            t,
            start_ns: state.now_nsec(),
        });
    }

    fn apply(&self) {
        while let Some(t) = self.apply.try_pop() {
            while let Some(member) = t.members.pop() {
                member.apply_dyn();
            }
            t.blockers.clear();
            self.unused_transactions.push(t);
        }
    }

    pub fn clear(&self, state: &Rc<State>) {
        self.commit(state, &mut Default::default(), TreeSerial::NONE);
        state.add_transaction_op(StateTransactionOp::Clear);
        self.commit(state, &mut Default::default(), TreeSerial::NONE);
        while let Some(t) = self.timeout.try_pop() {
            t.t.schedule_apply(t.version);
        }
        self.apply();

        self.surfaces.clear();
        self.scheduled.clear();
        self.unused_transactions.clear();
        self.apply.clear();
        self.timeout.clear();
        self.timeout_changed.clear();
        for t in self.all_transactions.take() {
            t.members.take();
            t.blockers.take();
        }
    }

    pub fn timeout_ns(&self) -> u64 {
        self.timeout_ns.get()
    }

    fn set_timeout_ns(&self, timeout: u64) {
        self.timeout_ns.set(timeout);
        self.timeout_changed.trigger();
    }
}

impl State {
    pub fn set_transaction_timeout_ns(&self, timeout: u64) {
        self.tree.transactions.set_timeout_ns(timeout);
        self.trigger_cci(CCI_COMPOSITOR);
    }
}

impl Drop for Blocker {
    fn drop(&mut self) {
        self.transaction.schedule_apply(self.version);
    }
}

impl Transaction {
    fn schedule_apply(self: &Rc<Self>, version: u64) {
        if self.version.get() != version {
            return;
        }
        self.version.fetch_add(1);
        self.state.tree.transactions.apply.push(self.clone());
    }
}

impl WlSurface {
    pub fn unblock_transactions_until(&self, serial: TreeSerial) {
        self.surface_transaction
            .unblock_transactions_until(&self.client.state, serial);
    }
}

impl SurfaceTransaction {
    fn unblock_transactions_until(&self, state: &Rc<State>, serial: TreeSerial) {
        if self.tardy.get() {
            if serial > self.unblocked_commit.get() {
                self.tardy.set(false);
            } else if serial == self.unblocked_commit.get() {
                let delta = state.now_nsec() - self.unblocked_commit_start_ns.get();
                if delta < state.tree.transactions.timeout_ns.get() {
                    self.tardy.set(false);
                }
            }
            if LOG_TARDY && !self.tardy.get() {
                log::warn!("marking not tardy");
            }
        }
        if let Some(first) = self.first_transaction_blocker.get()
            && first <= serial
        {
            while let Some(bt) = self.transaction_blockers.pop() {
                if bt.0 > serial {
                    self.first_transaction_blocker.set(Some(bt.0));
                    self.transaction_blockers.push_front(bt);
                    return;
                }
            }
            self.first_transaction_blocker.take();
        }
    }

    pub fn unblock_commits_until(&self, serial: TreeSerial, start_ns: u64) {
        if let Some(blocker) = self.first_transaction_blocker.get()
            && blocker <= serial
        {
            if LOG_TARDY && !self.tardy.get() {
                log::warn!("marking tardy");
            }
            self.tardy.set(true);
            self.transaction_blockers.clear();
            self.first_transaction_blocker.take();
        }
        self.unblocked_commit.set(serial);
        self.unblocked_commit_start_ns.set(start_ns);
    }

    pub fn commit_is_blocked(&self, serial: TreeSerial) -> bool {
        self.unblocked_commit.get() < serial
    }

    pub fn is_tardy(&self) -> bool {
        self.tardy.get()
    }
}

pub async fn handle_transactions_timeout(state: Rc<State>) {
    let ts = &state.tree.transactions;
    loop {
        let state2 = state.clone();
        let _timeout = state.eng.spawn("transactions timeout impl", async move {
            let ts = &state2.tree.transactions;
            let timeout_ns = ts.timeout_ns.get();
            loop {
                let t = ts.timeout.pop().await;
                let timeout_ns = t.start_ns.saturating_add(timeout_ns);
                if timeout_ns > state2.now_nsec() {
                    let push_back = on_drop(|| ts.timeout.push_front(t.clone()));
                    let res = state2.ring.timeout(timeout_ns).await;
                    push_back.forget();
                    if let Err(e) = res {
                        log::error!("Could not wait for transaction timeout: {}", ErrorFmt(e));
                    }
                }
                if LOG_TARDY && t.t.version.get() == t.version {
                    log::warn!("timed out");
                }
                t.t.schedule_apply(t.version);
            }
        });
        ts.timeout_changed.triggered().await;
    }
}

pub async fn handle_transactions_apply(state: Rc<State>) {
    let ts = &state.tree.transactions;
    loop {
        ts.apply.non_empty().await;
        ts.apply();
    }
}
