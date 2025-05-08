use {
    crate::criteria::{
        CritUpstreamNode,
        crit_graph::{
            CritDownstream, CritDownstreamData, CritTarget, CritUpstreamData,
            crit_target::CritMgr,
            crit_upstream::{CritUpstreamNodeBase, CritUpstreamNodeData},
        },
        crit_per_target_data::{CritDestroyListenerBase, CritPerTargetData},
    },
    std::rc::Rc,
};

pub struct CritMiddle<Target, T>
where
    Target: CritTarget,
    T: CritMiddleCriterion<Target>,
{
    upstream: CritDownstreamData<Target>,
    downstream: CritUpstreamData<Target, CritMiddleData<T::Data>>,
    criterion: T,
}

#[derive(Default)]
pub struct CritMiddleData<T> {
    matches: usize,
    data: T,
}

pub trait CritMiddleCriterion<Target>: Sized + 'static {
    type Data: Default;
    type Not: CritMiddleCriterion<Target>;

    fn update_matched(&self, target: &Target, node: &mut Self::Data, matched: bool) -> bool;
    fn pull(&self, upstream: &[Rc<dyn CritUpstreamNode<Target>>], target: &Target) -> bool;
    fn not(&self) -> Self::Not;
}

impl<Target, T> CritMiddle<Target, T>
where
    Target: CritTarget,
    T: CritMiddleCriterion<Target>,
{
    pub fn new(
        mgr: &Target::Mgr,
        upstream: &[Rc<dyn CritUpstreamNode<Target>>],
        criterion: T,
    ) -> Rc<Self> {
        let id = mgr.id();
        let slf = Rc::new_cyclic(|slf| Self {
            upstream: CritDownstreamData::new(id, upstream),
            downstream: CritUpstreamData::new(slf, id),
            criterion,
        });
        slf.upstream.attach(&slf);
        slf
    }
}

impl<Target, T> CritDownstream<Target> for CritMiddle<Target, T>
where
    Target: CritTarget,
    T: CritMiddleCriterion<Target>,
{
    fn update_matched(self: Rc<Self>, target: &Target, matched: bool) {
        let mut node = self.downstream.get_or_create(target);
        let change = self
            .criterion
            .update_matched(target, &mut node.data, matched);
        let matches = match matched {
            true => node.matches + 1,
            false => node.matches - 1,
        };
        node.matches = matches;
        self.downstream
            .update_matched(target, node, change, matches == 0);
    }
}

impl<Target, T> CritUpstreamNodeBase<Target> for CritMiddle<Target, T>
where
    Target: CritTarget,
    T: CritMiddleCriterion<Target>,
{
    type Data = CritMiddleData<T::Data>;

    fn data(&self) -> &CritUpstreamData<Target, Self::Data> {
        &self.downstream
    }

    fn not(&self, mgr: &Target::Mgr) -> Rc<dyn CritUpstreamNode<Target>> {
        let upstream = self.upstream.not(mgr);
        CritMiddle::new(mgr, &upstream, self.criterion.not())
    }

    fn pull(&self, target: &Target) -> bool {
        self.criterion.pull(&self.upstream.upstream, target)
    }
}

impl<Target, T> CritDestroyListenerBase<Target> for CritMiddle<Target, T>
where
    Target: CritTarget,
    T: CritMiddleCriterion<Target>,
{
    type Data = CritUpstreamNodeData<Target, CritMiddleData<T::Data>>;

    fn data(&self) -> &CritPerTargetData<Target, Self::Data> {
        &self.downstream.nodes
    }
}
