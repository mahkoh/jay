use crate::criteria::crit_graph::CritRootCriterion;
use crate::criteria::tlm::RootMatchers;
use crate::criteria::tlm::TlmRootMatcherMap;
use crate::ifs::wp_content_type_v1::ContentTypeExt;
use crate::tree::ToplevelData;
use crate::utils::bitflags::BitflagsExt;
use jay_config::window::ContentType;

pub struct TlmMatchContentType {
    kind: ContentType,
}

impl TlmMatchContentType {
    pub fn new(kind: ContentType) -> TlmMatchContentType {
        Self { kind }
    }
}

impl CritRootCriterion<ToplevelData> for TlmMatchContentType {
    fn matches(&self, data: &ToplevelData) -> bool {
        self.kind.0.contains(data.content_type.get().to_config().0)
    }

    fn nodes(roots: &RootMatchers) -> Option<&TlmRootMatcherMap<Self>> {
        Some(&roots.content_ty)
    }
}
