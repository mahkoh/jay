use {
    crate::criteria::{
        CritMatcherId,
        crit_graph::{CritTarget, CritTargetOwner, WeakCritTargetOwner},
    },
    ahash::AHashMap,
    std::{
        cell::{RefCell, RefMut},
        ops::{Deref, DerefMut},
        rc::Weak,
    },
};

pub struct CritPerTargetData<Target, T>
where
    Target: CritTarget,
{
    id: CritMatcherId,
    slf: Weak<dyn CritDestroyListener<Target>>,
    data: RefCell<AHashMap<Target::Id, PerTreeNodeData<Target, T>>>,
}

pub struct PerTreeNodeData<Target, T>
where
    Target: CritTarget,
{
    node: Target::Owner,
    data: T,
}

pub(super) trait CritDestroyListenerBase<Target>: 'static
where
    Target: CritTarget,
{
    type Data;

    fn data(&self) -> &CritPerTargetData<Target, Self::Data>;
}

pub trait CritDestroyListener<Target>: 'static
where
    Target: CritTarget,
{
    fn destroyed(&self, target_id: Target::Id);
}

impl<Target, T> CritPerTargetData<Target, T>
where
    Target: CritTarget,
{
    pub fn new(slf: &Weak<impl CritDestroyListener<Target>>, id: CritMatcherId) -> Self {
        Self {
            id,
            slf: slf.clone() as _,
            data: Default::default(),
        }
    }

    pub fn clear(&self) {
        self.data.borrow_mut().clear();
    }

    pub fn get_or_create(&self, target: &Target, default: impl FnOnce() -> T) -> RefMut<T> {
        RefMut::map(self.data.borrow_mut(), |d| {
            &mut d
                .entry(target.id())
                .or_insert_with(|| {
                    target.destroyed().set(self.id, self.slf.clone());
                    PerTreeNodeData {
                        node: target.owner(),
                        data: default(),
                    }
                })
                .data
        })
    }

    pub fn get(&self, target: &Target) -> Option<RefMut<T>> {
        RefMut::filter_map(self.data.borrow_mut(), |d| {
            d.get_mut(&target.id()).map(|d| &mut d.data)
        })
        .ok()
    }

    pub fn remove(&self, target_id: Target::Id) {
        if let Some(node) = self.data.borrow_mut().remove(&target_id)
            && let Some(node) = node.node.upgrade()
        {
            node.data().destroyed().remove(&self.id);
        }
    }

    pub fn borrow_mut(&self) -> RefMut<'_, AHashMap<Target::Id, PerTreeNodeData<Target, T>>> {
        self.data.borrow_mut()
    }
}

impl<Target, T> Drop for CritPerTargetData<Target, T>
where
    Target: CritTarget,
{
    fn drop(&mut self) {
        for d in self.data.borrow().values() {
            if let Some(n) = d.node.upgrade() {
                n.data().destroyed().remove(&self.id);
            }
        }
    }
}

impl<Target, T> CritDestroyListener<Target> for T
where
    Target: CritTarget,
    T: CritDestroyListenerBase<Target>,
{
    fn destroyed(&self, target_id: Target::Id) {
        let _v = self.data().data.borrow_mut().remove(&target_id);
    }
}

impl<Target, T> Deref for PerTreeNodeData<Target, T>
where
    Target: CritTarget,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<Target, T> DerefMut for PerTreeNodeData<Target, T>
where
    Target: CritTarget,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
