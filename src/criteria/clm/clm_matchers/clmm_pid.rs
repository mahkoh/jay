use {
    crate::{
        client::Client,
        criteria::{RootMatcherMap, clm::RootMatchers, crit_graph::CritRootCriterion},
    },
    std::rc::Rc,
    uapi::c,
};

pub struct ClmMatchPid(pub c::pid_t);

impl CritRootCriterion<Rc<Client>> for ClmMatchPid {
    fn matches(&self, data: &Rc<Client>) -> bool {
        data.pid_info.pid == self.0
    }

    fn nodes(roots: &RootMatchers) -> Option<&RootMatcherMap<Rc<Client>, Self>> {
        Some(&roots.pid)
    }
}
