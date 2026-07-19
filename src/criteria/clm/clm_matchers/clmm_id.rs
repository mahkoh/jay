use crate::client::Client;
use crate::client::ClientId;
use crate::criteria::RootMatcherMap;
use crate::criteria::clm::RootMatchers;
use crate::criteria::crit_graph::CritRootCriterion;
use std::rc::Rc;

pub struct ClmMatchId(pub ClientId);

impl CritRootCriterion<Rc<Client>> for ClmMatchId {
    fn matches(&self, data: &Rc<Client>) -> bool {
        data.id == self.0
    }

    fn nodes(roots: &RootMatchers) -> Option<&RootMatcherMap<Rc<Client>, Self>> {
        Some(&roots.id)
    }
}
