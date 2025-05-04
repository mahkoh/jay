use {
    crate::criteria::{
        CritMatcherId, CritUpstreamNode, FixedRootMatcher, RootMatcherMap,
        crit_graph::{
            CritTarget, CritUpstreamData,
            crit_target::CritMgr,
            crit_upstream::{CritUpstreamNodeBase, CritUpstreamNodeData},
        },
        crit_per_target_data::{CritDestroyListenerBase, CritPerTargetData},
    },
    std::{marker::PhantomData, rc::Rc},
};

pub struct CritRoot<Target, T>
where
    Target: CritTarget,
    T: CritRootCriterion<Target>,
{
    id: CritMatcherId,
    downstream: CritUpstreamData<Target, ()>,
    not: bool,
    criterion: Rc<T>,
    roots: Rc<Target::RootMatchers>,
}

pub struct CritRootFixed<Target, Crit>(pub Crit, pub PhantomData<fn(&Target)>);

pub trait CritRootCriterion<Target>: Sized + 'static
where
    Target: CritTarget,
{
    fn matches(&self, data: &Target) -> bool;
    fn nodes(roots: &Target::RootMatchers) -> Option<&RootMatcherMap<Target, Self>> {
        let _ = roots;
        None
    }
    fn not(&self, mgr: &Target::Mgr) -> Option<Rc<dyn CritUpstreamNode<Target>>> {
        let _ = mgr;
        None
    }
}

pub trait CritFixedRootCriterionBase<Target>: Sized + 'static
where
    Target: CritTarget,
{
    fn constant(&self) -> bool;
    fn not<'a>(&self, mgr: &'a Target::Mgr) -> &'a FixedRootMatcher<Target, Self>
    where
        Self: CritFixedRootCriterion<Target>;
}

pub trait CritFixedRootCriterion<Target>: CritFixedRootCriterionBase<Target>
where
    Target: CritTarget,
{
    const COMPARE: bool = true;

    fn matches(&self, data: &Target) -> bool;
}

impl<Target, T> CritRootCriterion<Target> for CritRootFixed<Target, T>
where
    Target: CritTarget,
    T: CritFixedRootCriterion<Target>,
{
    fn matches(&self, data: &Target) -> bool {
        let mut res = self.0.matches(data);
        if T::COMPARE {
            res = res == self.0.constant();
        }
        res
    }

    fn not(&self, mgr: &Target::Mgr) -> Option<Rc<dyn CritUpstreamNode<Target>>> {
        Some(self.0.not(mgr)[!self.0.constant()].clone())
    }
}

impl<Target, T> CritUpstreamNodeBase<Target> for CritRoot<Target, T>
where
    Target: CritTarget,
    T: CritRootCriterion<Target>,
{
    type Data = ();

    fn data(&self) -> &CritUpstreamData<Target, Self::Data> {
        &self.downstream
    }

    fn not(&self, mgr: &Target::Mgr) -> Rc<dyn CritUpstreamNode<Target>> {
        if let Some(node) = self.criterion.not(mgr) {
            return node;
        }
        let id = mgr.id();
        Self::new_(&self.roots, id, self.criterion.clone(), !self.not)
    }

    fn pull(&self, target: &Target) -> bool {
        self.criterion.matches(target) ^ self.not
    }
}

impl<Target, T> CritRoot<Target, T>
where
    Target: CritTarget,
    T: CritRootCriterion<Target>,
{
    pub fn new(roots: &Rc<Target::RootMatchers>, id: CritMatcherId, criterion: T) -> Rc<Self> {
        Self::new_(roots, id, Rc::new(criterion), false)
    }

    fn new_(
        roots: &Rc<Target::RootMatchers>,
        id: CritMatcherId,
        criterion: Rc<T>,
        not: bool,
    ) -> Rc<Self> {
        let slf = Rc::new_cyclic(|slf| Self {
            id,
            downstream: CritUpstreamData::new(slf, id),
            not,
            criterion,
            roots: roots.clone(),
        });
        if let Some(nodes) = T::nodes(roots) {
            nodes.set(id, Rc::downgrade(&slf));
        }
        slf
    }

    pub fn handle(&self, target: &Target) {
        let new = self.criterion.matches(target) ^ self.not;
        let node = match new {
            true => self.downstream.get_or_create(target),
            false => match self.downstream.get(target) {
                Some(n) => n,
                None => return,
            },
        };
        self.downstream.update_matched(target, node, new, !new);
    }

    #[expect(dead_code)]
    pub fn has_downstream(&self) -> bool {
        self.downstream.has_downstream()
    }
}

impl<Target, T> CritDestroyListenerBase<Target> for CritRoot<Target, T>
where
    Target: CritTarget,
    T: CritRootCriterion<Target>,
{
    type Data = CritUpstreamNodeData<Target, ()>;

    fn data(&self) -> &CritPerTargetData<Target, Self::Data> {
        &self.downstream.nodes
    }
}

impl<Target, T> Drop for CritRoot<Target, T>
where
    Target: CritTarget,
    T: CritRootCriterion<Target>,
{
    fn drop(&mut self) {
        if let Some(nodes) = T::nodes(&self.roots) {
            nodes.remove(&self.id);
        }
    }
}
