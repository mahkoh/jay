use crate::criteria::crit_graph::CritFixedRootCriterion;
use crate::tree::ToplevelData;

pub struct TlmMatchUrgent(pub bool);

fixed_root_criterion!(TlmMatchUrgent, urgent);

impl CritFixedRootCriterion<ToplevelData> for TlmMatchUrgent {
    fn matches(&self, data: &ToplevelData) -> bool {
        data.wants_attention.get()
    }
}
