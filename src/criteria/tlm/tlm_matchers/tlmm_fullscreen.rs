use crate::{
    criteria::crit_graph::CritFixedRootCriterion,
    tree::{ToplevelData, TreeTimeline::LiveTL},
};

pub struct TlmMatchFullscreen(pub bool);

fixed_root_criterion!(TlmMatchFullscreen, fullscreen);

impl CritFixedRootCriterion<ToplevelData> for TlmMatchFullscreen {
    fn matches(&self, data: &ToplevelData) -> bool {
        data.is_fullscreen[LiveTL].get()
    }
}
