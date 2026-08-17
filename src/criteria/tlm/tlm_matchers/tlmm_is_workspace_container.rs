use crate::criteria::crit_graph::CritFixedRootCriterion;
use crate::tree::ToplevelData;
use crate::tree::TreeTimeline::LiveTL;

pub struct TlmMatchIsWorkspaceContainer(pub bool);

fixed_root_criterion!(TlmMatchIsWorkspaceContainer, is_workspace_container);

impl CritFixedRootCriterion<ToplevelData> for TlmMatchIsWorkspaceContainer {
    fn matches(&self, data: &ToplevelData) -> bool {
        data.is_root_container[LiveTL].get()
    }
}
