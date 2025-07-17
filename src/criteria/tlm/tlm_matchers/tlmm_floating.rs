use crate::{criteria::crit_graph::CritFixedRootCriterion, tree::ToplevelData};

pub struct TlmMatchFloating(pub bool);

fixed_root_criterion!(TlmMatchFloating, floating);

impl CritFixedRootCriterion<ToplevelData> for TlmMatchFloating {
    fn matches(&self, data: &ToplevelData) -> bool {
        data.parent_is_float.get()
    }
}
