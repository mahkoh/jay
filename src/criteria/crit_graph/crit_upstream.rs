use {
    crate::{
        criteria::{
            CritDestroyListener, CritMatcherId,
            crit_graph::{
                WeakCritTargetOwner,
                crit_downstream::CritDownstream,
                crit_target::{CritTarget, CritTargetOwner},
            },
            crit_per_target_data::CritPerTargetData,
        },
        utils::copyhashmap::CopyHashMap,
    },
    std::{
        cell::RefMut,
        mem,
        ops::{Deref, DerefMut},
        rc::{Rc, Weak},
    },
};

pub struct CritUpstreamData<Target, T>
where
    Target: CritTarget,
{
    downstream: CopyHashMap<CritMatcherId, Weak<dyn CritDownstream<Target>>>,
    pub nodes: CritPerTargetData<Target, CritUpstreamNodeData<Target, T>>,
}

pub struct CritUpstreamNodeData<Target, T>
where
    Target: CritTarget,
{
    matched: bool,
    tl: Target::Owner,
    data: T,
}

pub trait CritUpstreamNodeBase<Target>: 'static
where
    Target: CritTarget,
{
    type Data;

    fn data(&self) -> &CritUpstreamData<Target, Self::Data>;
    fn not(&self, mgr: &Target::Mgr) -> Rc<dyn CritUpstreamNode<Target>>;
    fn pull(&self, target: &Target) -> bool;
}

pub trait CritUpstreamNode<Target>: 'static
where
    Target: CritTarget,
{
    fn attach(&self, id: CritMatcherId, downstream: Rc<dyn CritDownstream<Target>>);
    fn detach(&self, id: CritMatcherId);
    fn not(&self, mgr: &Target::Mgr) -> Rc<dyn CritUpstreamNode<Target>>;
    fn pull(&self, target: &Target) -> bool;
    fn get(&self, target: &Target) -> bool;
}

impl<Target, T> CritUpstreamNode<Target> for T
where
    Target: CritTarget,
    T: CritUpstreamNodeBase<Target>,
{
    fn attach(&self, id: CritMatcherId, downstream: Rc<dyn CritDownstream<Target>>) {
        let data = self.data();
        for n in data.nodes.borrow_mut().values_mut() {
            if !n.matched {
                continue;
            }
            let Some(target) = n.tl.upgrade() else {
                continue;
            };
            downstream.clone().update_matched(target.data(), true);
        }
        data.downstream.set(id, Rc::downgrade(&downstream));
    }

    fn detach(&self, id: CritMatcherId) {
        self.data().downstream.remove(&id);
    }

    fn not(&self, mgr: &Target::Mgr) -> Rc<dyn CritUpstreamNode<Target>> {
        <T as CritUpstreamNodeBase<Target>>::not(self, mgr)
    }

    fn pull(&self, target: &Target) -> bool {
        <T as CritUpstreamNodeBase<Target>>::pull(self, target)
    }

    fn get(&self, target: &Target) -> bool {
        <T as CritUpstreamNodeBase<Target>>::data(self).matched(target)
    }
}

impl<Target, T> Deref for CritUpstreamNodeData<Target, T>
where
    Target: CritTarget,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<Target, T> DerefMut for CritUpstreamNodeData<Target, T>
where
    Target: CritTarget,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<Target, T> CritUpstreamData<Target, T>
where
    Target: CritTarget,
{
    pub fn new(slf: &Weak<impl CritDestroyListener<Target>>, id: CritMatcherId) -> Self {
        Self {
            downstream: Default::default(),
            nodes: CritPerTargetData::new(slf, id),
        }
    }

    pub fn clear(&self) {
        self.nodes.clear()
    }

    pub fn update_matched(
        &self,
        target: &Target,
        mut node: RefMut<CritUpstreamNodeData<Target, T>>,
        matched: bool,
        remove: bool,
    ) {
        let unchanged = mem::replace(&mut node.matched, matched) == matched;
        drop(node);
        if remove {
            self.nodes.remove(target.id());
        }
        if unchanged {
            return;
        }
        for el in self.downstream.lock().values() {
            if let Some(el) = el.upgrade() {
                el.update_matched(target, matched);
            }
        }
    }

    pub fn get_or_create(&self, target: &Target) -> RefMut<CritUpstreamNodeData<Target, T>>
    where
        T: Default,
    {
        self.nodes.get_or_create(target, || CritUpstreamNodeData {
            matched: false,
            tl: target.owner(),
            data: Default::default(),
        })
    }

    pub fn get(&self, target: &Target) -> Option<RefMut<CritUpstreamNodeData<Target, T>>> {
        self.nodes.get(target)
    }

    pub fn has_downstream(&self) -> bool {
        self.downstream.is_not_empty()
    }

    pub fn matched(&self, target: &Target) -> bool {
        let Some(node) = self.nodes.get(target) else {
            return false;
        };
        node.matched
    }
}
