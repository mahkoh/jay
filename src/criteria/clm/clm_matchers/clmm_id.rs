use {
    crate::{
        client::{Client, ClientId},
        criteria::{RootMatcherMap, clm::RootMatchers, crit_graph::CritRootCriterion},
    },
    std::rc::Rc,
};

pub struct ClmMatchId(pub ClientId);

impl CritRootCriterion<Rc<Client>> for ClmMatchId {
    fn matches(&self, data: &Rc<Client>) -> bool {
        data.id == self.0
    }

    fn nodes(roots: &RootMatchers) -> Option<&RootMatcherMap<Rc<Client>, Self>> {
        Some(&roots.id)
    }
}
