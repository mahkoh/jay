use crate::client::Client;
use crate::criteria::RootMatcherMap;
use crate::criteria::clm::RootMatchers;
use crate::criteria::crit_graph::CritRootCriterion;
use std::rc::Rc;
use uapi::c;

pub struct ClmMatchUid(pub c::uid_t);

impl CritRootCriterion<Rc<Client>> for ClmMatchUid {
    fn matches(&self, data: &Rc<Client>) -> bool {
        data.pid_info.uid == self.0
    }

    fn nodes(roots: &RootMatchers) -> Option<&RootMatcherMap<Rc<Client>, Self>> {
        Some(&roots.uid)
    }
}
