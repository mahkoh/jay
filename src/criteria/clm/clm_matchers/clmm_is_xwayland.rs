use crate::client::Client;
use crate::criteria::crit_graph::CritFixedRootCriterion;
use std::rc::Rc;

pub struct ClmMatchIsXwayland(pub bool);

fixed_root_criterion!(ClmMatchIsXwayland, is_xwayland);

impl CritFixedRootCriterion<Rc<Client>> for ClmMatchIsXwayland {
    fn matches(&self, data: &Rc<Client>) -> bool {
        data.is_xwayland
    }
}
