use {
    crate::{
        criteria::{
            crit_graph::CritRootCriterion,
            tlm::{RootMatchers, TlmRootMatcherMap},
        },
        ifs::wp_content_type_v1::ContentTypeExt,
        tree::ToplevelData,
        utils::bitflags::BitflagsExt,
    },
    jay_config::window::ContentType,
};

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
