use crate::{
    criteria::{
        crit_graph::CritRootCriterion,
        tlm::{RootMatchers, TlmRootMatcherMap},
    },
    tree::ToplevelData,
    utils::toplevel_identifier::ToplevelIdentifier,
};

pub struct TlmMatchIdentifier {
    id: ToplevelIdentifier,
}

impl TlmMatchIdentifier {
    pub fn new(id: ToplevelIdentifier) -> TlmMatchIdentifier {
        Self { id }
    }
}

impl CritRootCriterion<ToplevelData> for TlmMatchIdentifier {
    fn matches(&self, data: &ToplevelData) -> bool {
        data.identifier.get() == self.id
    }

    fn nodes(roots: &RootMatchers) -> Option<&TlmRootMatcherMap<Self>> {
        Some(&roots.identifiers)
    }
}
