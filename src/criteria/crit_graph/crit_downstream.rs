use {
    crate::criteria::{
        CritMatcherId,
        crit_graph::{CritTarget, crit_upstream::CritUpstreamNode},
    },
    std::rc::Rc,
};

pub struct CritDownstreamData<Target>
where
    Target: CritTarget,
{
    id: CritMatcherId,
    pub(super) upstream: Vec<Rc<dyn CritUpstreamNode<Target>>>,
}

pub trait CritDownstream<Target>: 'static {
    fn update_matched(self: Rc<Self>, target: &Target, matched: bool);
}

impl<Target> CritDownstreamData<Target>
where
    Target: CritTarget,
{
    pub fn new(id: CritMatcherId, upstream: &[Rc<dyn CritUpstreamNode<Target>>]) -> Self {
        Self {
            id,
            upstream: upstream.to_vec(),
        }
    }

    pub fn attach(&self, slf: &Rc<impl CritDownstream<Target>>) {
        for upstream in &self.upstream {
            upstream.attach(self.id, slf.clone() as _);
        }
    }

    pub fn not(&self, mgr: &Target::Mgr) -> Vec<Rc<dyn CritUpstreamNode<Target>>> {
        self.upstream.iter().map(|n| n.not(mgr)).collect()
    }
}

impl<Target> Drop for CritDownstreamData<Target>
where
    Target: CritTarget,
{
    fn drop(&mut self) {
        for el in &self.upstream {
            el.detach(self.id);
        }
    }
}
