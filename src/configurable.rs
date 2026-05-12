use {
    crate::{
        state::State,
        tree::TreeSerial,
        utils::{
            numcell::NumCell,
            stack::{AsyncStack, Stack},
        },
    },
    isnt::std_1::vec::IsntVecExt,
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        mem,
        rc::Rc,
    },
};

pub trait Configurable: 'static {
    type T: Sized;

    fn data(&self) -> &ConfigurableData<Self::T>;
    fn configure_data(&self) -> Self::T;
    fn merge(first: &mut Self::T, second: Self::T);
    fn destroyed(&self) -> bool;
    fn flush(&self, serial: TreeSerial, data: Self::T);
}

#[expect(dead_code)]
pub trait ConfigurableExt: Configurable {
    fn schedule_configure(self: &Rc<Self>);
}

trait ConfigurableDyn {
    fn data(&self) -> &ConfigurableDataCore;
    fn schedule(
        self: Rc<Self>,
        group: &Rc<ConfigureGroup>,
        members: &mut Vec<Rc<dyn ConfigurableDyn>>,
    );
    fn flush(&self, nr: usize, serial: TreeSerial);
}

#[derive(Default)]
pub struct ConfigureGroups {
    scheduled: AsyncStack<Rc<dyn ConfigurableDyn>>,
    ready: AsyncStack<Rc<dyn ConfigurableDyn>>,
    all_groups: Stack<Rc<ConfigureGroup>>,
    unused_groups: Stack<Rc<ConfigureGroup>>,
    groups_to_recycle: Stack<Rc<ConfigureGroup>>,
}

pub struct ConfigurableData<T> {
    core: ConfigurableDataCore,
    requests: RefCell<VecDeque<T>>,
}

pub struct ConfigurableDataCore {
    state: Rc<State>,
    scheduled: Cell<bool>,
    idle: Cell<bool>,
    requests: RefCell<VecDeque<Rc<ConfigureGroup>>>,

    // dirty
    num_ready: NumCell<usize>,
    iteration: Cell<u64>,
}

struct ConfigureGroup {
    serial: Cell<TreeSerial>,
    num_not_ready: NumCell<usize>,
    members: RefCell<Vec<Rc<dyn ConfigurableDyn>>>,

    // dirty
    iteration: Cell<u64>,
    num_not_ready2: NumCell<usize>,
}

impl<T> ConfigurableExt for T
where
    T: Configurable,
{
    fn schedule_configure(self: &Rc<Self>) {
        let d = self.data();
        if d.core.scheduled.replace(true) {
            return;
        }
        let cgs = &d.core.state.configure_groups;
        cgs.scheduled.push(self.clone());
    }
}

impl<T> ConfigurableData<T> {
    #[expect(dead_code)]
    pub fn new(state: &Rc<State>) -> Self {
        Self {
            core: ConfigurableDataCore {
                state: state.clone(),
                scheduled: Default::default(),
                idle: Cell::new(true),
                requests: Default::default(),
                num_ready: Default::default(),
                iteration: Default::default(),
            },
            requests: Default::default(),
        }
    }
}

impl ConfigureGroups {
    pub fn clear(&self) {
        self.scheduled.clear();
        self.ready.clear();
        self.unused_groups.take();
        self.groups_to_recycle.take();
        for group in self.all_groups.take() {
            group.members.take();
        }
    }
}

impl ConfigurableDataCore {
    fn ready(&self) {
        if self.idle.replace(true) {
            return;
        }
        let queue = &*self.requests.borrow();
        let Some(cg) = queue.front() else {
            return;
        };
        if cg.num_not_ready.sub_fetch(1) > 0 {
            return;
        }
        let cgs = &self.state.configure_groups;
        cgs.groups_to_recycle.push(cg.clone());
        for member in &*cg.members.borrow() {
            cgs.ready.push(member.clone());
        }
    }
}

impl<T> ConfigurableDyn for T
where
    T: Configurable,
{
    fn data(&self) -> &ConfigurableDataCore {
        &self.data().core
    }

    fn schedule(
        self: Rc<Self>,
        group: &Rc<ConfigureGroup>,
        members: &mut Vec<Rc<dyn ConfigurableDyn>>,
    ) {
        {
            let d = self.data();
            let requests = &mut *d.requests.borrow_mut();
            let core_requests = &mut *d.core.requests.borrow_mut();
            if !d.core.idle.get() || core_requests.len() > 0 {
                group.num_not_ready.fetch_add(1);
            }
            core_requests.push_back(group.clone());
            requests.push_back(self.configure_data());
            d.core.scheduled.set(false);
        }
        members.push(self);
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
        d.core.ready();
        if !self.destroyed() {
            self.flush(serial, data);
        }
    }
}

pub async fn handle_configurables_commit(state: Rc<State>) {
    let cgs = &state.configure_groups;
    let mut scheduled = vec![];
    loop {
        cgs.scheduled.non_empty().await;
        cgs.scheduled.swap(&mut scheduled);
        if scheduled.is_empty() {
            continue;
        }
        let serial = state.next_tree_serial();
        let cg = match cgs.unused_groups.pop() {
            Some(i) => i,
            _ => {
                let group = Rc::new(ConfigureGroup {
                    serial: Cell::new(serial),
                    num_not_ready: Default::default(),
                    members: Default::default(),
                    iteration: Default::default(),
                    num_not_ready2: Default::default(),
                });
                cgs.all_groups.push(group.clone());
                group
            }
        };
        cg.serial.set(serial);
        cg.num_not_ready.set(0);
        let members = &mut *cg.members.borrow_mut();
        while let Some(configurable) = scheduled.pop() {
            configurable.schedule(&cg, members);
        }
        if members.is_empty() {
            cgs.unused_groups.push(cg.clone());
            continue;
        }
        if cg.num_not_ready.get() > 0 {
            continue;
        }
        cgs.groups_to_recycle.push(cg.clone());
        for member in members {
            cgs.ready.push(member.clone());
        }
    }
}

pub async fn handle_configurables_apply(state: Rc<State>) {
    let cgs = &state.configure_groups;
    let ready = &cgs.ready;
    let to_recycle = &cgs.groups_to_recycle;
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
            cgs,
            &mut iteration,
            &mut all_with_ready,
            &mut of_interest_1,
            &mut of_interest_2,
            &mut groups_to_recycle,
        );
    }
}

fn run_iteration(
    cgs: &ConfigureGroups,
    iteration: &mut u64,
    all_with_ready: &mut Vec<Rc<dyn ConfigurableDyn>>,
    of_interest_1: &mut Vec<Rc<dyn ConfigurableDyn>>,
    of_interest_2: &mut Vec<Rc<dyn ConfigurableDyn>>,
    groups_to_recycle: &mut Vec<Rc<ConfigureGroup>>,
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
            let cg = &r[nr];
            if cg.iteration.replace(iteration) != iteration {
                cg.num_not_ready2.set(cg.num_not_ready.get());
            }
            if cg.num_not_ready2.sub_fetch(1) > 0 {
                continue;
            }
            groups_to_recycle.push(cg.clone());
            for member in &*cg.members.borrow() {
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
    while let Some(member) = all_with_ready.pop() {
        let d = member.data();
        d.idle.set(false);
        let nr = d.num_ready.get();
        let serial = d
            .requests
            .borrow_mut()
            .drain(..nr)
            .map(|r| r.serial.get())
            .max()
            .unwrap();
        member.flush(nr, serial);
    }
    for group in groups_to_recycle.drain(..) {
        group.members.borrow_mut().clear();
        cgs.unused_groups.push(group)
    }
}
