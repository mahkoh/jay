use {
    crate::{
        state::State,
        tree::TreeSerial,
        utils::{
            asyncevent::AsyncEvent,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            on_drop::OnDrop,
            queue::AsyncQueue,
            stack::{AsyncStack, Stack},
        },
    },
    isnt::std_1::vec::IsntVecExt,
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        mem,
        ops::Deref,
        rc::Rc,
    },
};

pub trait Configurable: 'static {
    type T: Sized;

    fn data(&self) -> &ConfigurableData<Self::T>;
    fn merge(first: &mut Self::T, second: Self::T);
    fn visible(&self) -> bool;
    fn destroyed(&self) -> bool;
    fn flush(&self, serial: TreeSerial, data: Self::T);
}

trait ConfigurableDyn {
    fn data(&self) -> &ConfigurableDataCore;
    fn flush(&self, nr: usize, serial: TreeSerial);
}

pub struct ConfigureGroups {
    ready: AsyncStack<Rc<dyn ConfigurableDyn>>,
    all_groups: Stack<Rc<ConfigureGroupInner>>,
    unused_groups: Stack<Rc<ConfigureGroupInner>>,
    groups_to_recycle: Stack<Rc<ConfigureGroupInner>>,
    timeout: AsyncQueue<ConfigurableTimeout>,
    timeout_ns: Cell<u64>,
    timeout_changed: AsyncEvent,
}

pub struct ConfigurableData<T> {
    core: ConfigurableDataCore,
    requests: RefCell<VecDeque<T>>,
}

pub struct ConfigurableDataCore {
    idle: Cell<bool>,
    num_idle_calls: NumCell<u64>,
    last_busy_ns: Cell<u64>,
    requests: RefCell<VecDeque<Rc<ConfigureGroupInner>>>,
    largest_serial: Cell<TreeSerial>,
    tardy: Cell<bool>,

    // dirty
    num_ready: NumCell<usize>,
    iteration: Cell<u64>,
}

pub struct ConfigureGroup {
    inner: Rc<ConfigureGroupInner>,
}

struct ConfigureGroupInner {
    serial: Cell<TreeSerial>,
    num_not_ready: NumCell<usize>,
    members: RefCell<Vec<Rc<dyn ConfigurableDyn>>>,
    groups: Rc<ConfigureGroups>,

    // dirty
    iteration: Cell<u64>,
    num_not_ready2: NumCell<usize>,
}

#[derive(Clone)]
struct ConfigurableTimeout {
    configurable: Rc<dyn ConfigurableDyn>,
    num_idle_calls: u64,
}

impl<T> Default for ConfigurableData<T> {
    fn default() -> Self {
        Self {
            core: Default::default(),
            requests: Default::default(),
        }
    }
}

impl<T> Deref for ConfigurableData<T> {
    type Target = ConfigurableDataCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl Default for ConfigurableDataCore {
    fn default() -> Self {
        Self {
            idle: Cell::new(true),
            num_idle_calls: Default::default(),
            last_busy_ns: Default::default(),
            requests: Default::default(),
            largest_serial: Cell::new(TreeSerial::from_raw(0)),
            tardy: Cell::new(false),
            num_ready: Default::default(),
            iteration: Default::default(),
        }
    }
}

// pub const DEFAULT_TIMEOUT_NS: u64 = 100_000_000_000;
pub const DEFAULT_TIMEOUT_NS: u64 = 3_000_000_000;

impl Default for ConfigureGroups {
    fn default() -> Self {
        Self {
            ready: Default::default(),
            all_groups: Default::default(),
            unused_groups: Default::default(),
            groups_to_recycle: Default::default(),
            timeout: Default::default(),
            timeout_ns: Cell::new(DEFAULT_TIMEOUT_NS),
            timeout_changed: Default::default(),
        }
    }
}

impl ConfigureGroups {
    pub fn clear(&self) {
        self.ready.clear();
        self.unused_groups.take();
        self.groups_to_recycle.take();
        self.timeout.clear();
        self.timeout_changed.clear();
        for group in self.all_groups.take() {
            group.members.take();
        }
    }

    pub fn group(self: &Rc<Self>, serial: TreeSerial) -> ConfigureGroup {
        let inner = match self.unused_groups.pop() {
            Some(i) => i,
            _ => {
                let i = Rc::new(ConfigureGroupInner {
                    serial: Cell::new(serial),
                    num_not_ready: Default::default(),
                    members: Default::default(),
                    groups: self.clone(),
                    iteration: Default::default(),
                    num_not_ready2: Default::default(),
                });
                self.all_groups.push(i.clone());
                i
            }
        };
        inner.serial.set(serial);
        inner.num_not_ready.set(0);
        ConfigureGroup { inner }
    }
}

impl Drop for ConfigureGroup {
    fn drop(&mut self) {
        if self.inner.num_not_ready.get() > 0 {
            return;
        }
        for member in &*self.inner.members.borrow() {
            self.inner.groups.ready.push(member.clone());
        }
    }
}

impl ConfigurableDataCore {
    pub fn ready(&self) {
        if self.idle.replace(true) {
            return;
        }
        self.num_idle_calls.fetch_add(1);
        let r = &*self.requests.borrow();
        let Some(r) = r.front() else {
            return;
        };
        if r.num_not_ready.fetch_sub(1) > 1 {
            return;
        }
        r.groups.groups_to_recycle.push(r.clone());
        for member in &*r.members.borrow() {
            r.groups.ready.push(member.clone());
        }
    }

    #[expect(dead_code)]
    pub fn enable_tardy(&self) {
        self.tardy.set(true);
        self.ready();
    }

    #[expect(dead_code)]
    pub fn disable_tardy(&self) {
        self.tardy.set(false);
    }
}

impl ConfigureGroup {
    pub fn add<C>(&self, configurable: &Rc<C>, data: C::T)
    where
        C: Configurable,
    {
        let d = configurable.data();
        let requests = &mut *d.requests.borrow_mut();
        let serial = self.inner.serial.get();
        if d.core.largest_serial.replace(serial) == serial
            && let Some(last) = requests.back_mut()
        {
            C::merge(last, data);
            return;
        }
        let core_requests = &mut *d.core.requests.borrow_mut();
        if !d.core.idle.get() || core_requests.len() > 0 {
            self.inner.num_not_ready.fetch_add(1);
        }
        core_requests.push_back(self.inner.clone());
        requests.push_back(data);
        self.inner.members.borrow_mut().push(configurable.clone());
    }
}

impl<T> ConfigurableDyn for T
where
    T: Configurable,
{
    fn data(&self) -> &ConfigurableDataCore {
        &self.data().core
    }

    fn flush(&self, nr: usize, serial: TreeSerial) {
        let d = self.data();
        let data = {
            let requests = &mut *d.requests.borrow_mut();
            let mut data = requests.pop_front().unwrap();
            for _ in 0..nr - 1 {
                T::merge(&mut data, requests.pop_front().unwrap());
            }
            data
        };
        if self.destroyed() {
            d.ready();
        } else {
            if !self.visible() {
                d.ready();
            }
            self.flush(serial, data);
        }
    }
}

pub async fn handle_configurables_timeout(state: Rc<State>) {
    let i = &*state.configure_groups;
    loop {
        let state2 = state.clone();
        let _timeout = state.eng.spawn("configurables timeout impl", async move {
            let i = &*state2.configure_groups;
            let timeout_ns = i.timeout_ns.get();
            loop {
                let t = i.timeout.pop().await;
                let d = t.configurable.data();
                let timeout_ns = d.last_busy_ns.get() + timeout_ns;
                if timeout_ns > state2.now_nsec() {
                    let push_back = OnDrop(|| i.timeout.push_front(t.clone()));
                    let res = state2.ring.timeout(timeout_ns).await;
                    push_back.forget();
                    if let Err(e) = res {
                        log::error!("Could not wait for configurable timeout: {}", ErrorFmt(e));
                    }
                }
                if t.num_idle_calls == d.num_idle_calls.get() {
                    d.ready();
                }
            }
        });
        i.timeout_changed.triggered().await;
    }
}

pub async fn handle_configurables(state: Rc<State>) {
    let inner = &state.configure_groups;
    let ready = &inner.ready;
    let to_recycle = &inner.groups_to_recycle;
    let mut all_with_ready = vec![];
    let mut of_interest_1 = vec![];
    let mut of_interest_2 = vec![];
    let mut groups_to_recycle = vec![];
    let mut iteration = 0;
    loop {
        ready.non_empty().await;
        ready.swap(&mut all_with_ready);
        to_recycle.swap(&mut groups_to_recycle);
        run_iteration(
            &state,
            inner,
            &mut iteration,
            &mut all_with_ready,
            &mut of_interest_1,
            &mut of_interest_2,
            &mut groups_to_recycle,
        );
    }
}

fn run_iteration(
    state: &State,
    inner: &ConfigureGroups,
    iteration: &mut u64,
    all_with_ready: &mut Vec<Rc<dyn ConfigurableDyn>>,
    of_interest_1: &mut Vec<Rc<dyn ConfigurableDyn>>,
    of_interest_2: &mut Vec<Rc<dyn ConfigurableDyn>>,
    groups_to_recycle: &mut Vec<Rc<ConfigureGroupInner>>,
) {
    of_interest_1.extend(all_with_ready.iter().cloned());
    *iteration += 1;
    let iteration = *iteration;
    for c in &**all_with_ready {
        let d = c.data();
        d.num_ready.set(1);
        d.iteration.set(iteration);
    }
    while of_interest_1.is_not_empty() {
        for c in &**of_interest_1 {
            let d = c.data();
            let r = &*d.requests.borrow();
            let nr = d.num_ready.get();
            if nr >= r.len() {
                continue;
            }
            let cgi = &r[nr];
            if cgi.iteration.replace(iteration) != iteration {
                cgi.num_not_ready2.set(cgi.num_not_ready.get());
            }
            if cgi.num_not_ready2.fetch_sub(1) > 1 {
                continue;
            }
            groups_to_recycle.push(cgi.clone());
            for member in &*cgi.members.borrow() {
                of_interest_2.push(member.clone());
                let d = member.data();
                if d.iteration.replace(iteration) != iteration {
                    d.num_ready.set(1);
                    all_with_ready.push(member.clone());
                } else {
                    d.num_ready.fetch_add(1);
                }
            }
        }
        of_interest_1.clear();
        mem::swap(of_interest_1, of_interest_2);
    }
    let now_ns = state.now_nsec();
    while let Some(member) = all_with_ready.pop() {
        let d = member.data();
        d.idle.set(false);
        d.last_busy_ns.set(now_ns);
        let nr = d.num_ready.get();
        let serial = d
            .requests
            .borrow_mut()
            .drain(..nr)
            .map(|r| r.serial.get())
            .max()
            .unwrap();
        member.flush(nr, serial);
        if d.tardy.get() {
            d.ready();
        } else {
            inner.timeout.push(ConfigurableTimeout {
                num_idle_calls: d.num_idle_calls.get(),
                configurable: member,
            });
        }
    }
    for group in groups_to_recycle.drain(..) {
        group.members.borrow_mut().clear();
        inner.unused_groups.push(group)
    }
}
