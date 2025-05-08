use {
    crate::{
        criteria::{
            crit_graph::CritRootCriterion,
            tlm::{RootMatchers, TlmRootMatcherMap},
        },
        tree::ToplevelData,
        utils::bitflags::BitflagsExt,
    },
    jay_config::window::WindowType,
};

pub struct TlmMatchKind {
    kind: WindowType,
}

impl TlmMatchKind {
    pub fn new(kind: WindowType) -> TlmMatchKind {
        Self { kind }
    }
}

impl CritRootCriterion<ToplevelData> for TlmMatchKind {
    fn matches(&self, data: &ToplevelData) -> bool {
        self.kind.0.contains(data.kind.to_window_type().0)
    }

    fn nodes(roots: &RootMatchers) -> Option<&TlmRootMatcherMap<Self>> {
        Some(&roots.kinds)
    }
}
