use {
    crate::{client::Client, criteria::crit_graph::CritFixedRootCriterion},
    std::rc::Rc,
};

pub struct ClmMatchSandboxed(pub bool);

fixed_root_criterion!(ClmMatchSandboxed, sandboxed);

impl CritFixedRootCriterion<Rc<Client>> for ClmMatchSandboxed {
    fn matches(&self, data: &Rc<Client>) -> bool {
        data.acceptor.sandboxed
    }
}
