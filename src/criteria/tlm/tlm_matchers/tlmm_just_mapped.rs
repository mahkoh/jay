use crate::{criteria::crit_graph::CritFixedRootCriterion, tree::ToplevelData};

pub struct TlmMatchJustMapped(pub bool);

fixed_root_criterion!(TlmMatchJustMapped, just_mapped);

impl CritFixedRootCriterion<ToplevelData> for TlmMatchJustMapped {
    fn matches(&self, data: &ToplevelData) -> bool {
        data.just_mapped()
    }
}
