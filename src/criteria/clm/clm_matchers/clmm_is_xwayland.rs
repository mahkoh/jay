use {
    crate::{client::Client, criteria::crit_graph::CritFixedRootCriterion},
    std::rc::Rc,
};

pub struct ClmMatchIsXwayland(pub bool);

fixed_root_criterion!(ClmMatchIsXwayland, is_xwayland);

impl CritFixedRootCriterion<Rc<Client>> for ClmMatchIsXwayland {
    fn matches(&self, data: &Rc<Client>) -> bool {
        data.is_xwayland
    }
}
