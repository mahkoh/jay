use {
    crate::{
        client::Client,
        criteria::{RootMatcherMap, clm::RootMatchers, crit_graph::CritRootCriterion},
    },
    std::rc::Rc,
    uapi::c,
};

pub struct ClmMatchUid(pub c::uid_t);

impl CritRootCriterion<Rc<Client>> for ClmMatchUid {
    fn matches(&self, data: &Rc<Client>) -> bool {
        data.pid_info.uid == self.0
    }

    fn nodes(roots: &RootMatchers) -> Option<&RootMatcherMap<Rc<Client>, Self>> {
        Some(&roots.uid)
    }
}
