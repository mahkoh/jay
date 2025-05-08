use crate::{criteria::crit_graph::CritFixedRootCriterion, tree::ToplevelData};

pub struct TlmMatchVisible(pub bool);

fixed_root_criterion!(TlmMatchVisible, visible);

impl CritFixedRootCriterion<ToplevelData> for TlmMatchVisible {
    fn matches(&self, data: &ToplevelData) -> bool {
        data.visible.get()
    }
}
