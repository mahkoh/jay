pub mod clm;
mod crit_graph;
pub mod crit_leaf;
mod crit_matchers;
mod crit_per_target_data;
pub mod tlm;

use {
    crate::{
        criteria::{
            crit_graph::{CritMgr, CritMiddle, CritRoot, CritRootCriterion, CritRootFixed},
            crit_leaf::CritLeafMatcher,
            crit_matchers::{critm_any_or_all::CritMatchAnyOrAll, critm_exactly::CritMatchExactly},
        },
        utils::copyhashmap::CopyHashMap,
    },
    linearize::StaticMap,
    regex::Regex,
    std::rc::{Rc, Weak},
};
pub use {
    crit_graph::{CritTarget, CritUpstreamNode},
    crit_per_target_data::CritDestroyListener,
};

linear_ids!(CritMatcherIds, CritMatcherId, u64);

type RootMatcherMap<Target, T> = CopyHashMap<CritMatcherId, Weak<CritRoot<Target, T>>>;
type FixedRootMatcher<Target, T> = StaticMap<bool, Rc<CritRoot<Target, CritRootFixed<Target, T>>>>;

#[derive(Clone)]
pub enum CritLiteralOrRegex {
    Literal(String),
    Regex(Regex),
}

impl CritLiteralOrRegex {
    fn matches(&self, string: &str) -> bool {
        match self {
            CritLiteralOrRegex::Literal(p) => string == p,
            CritLiteralOrRegex::Regex(r) => r.is_match(string),
        }
    }
}

pub trait CritMgrExt: CritMgr {
    fn list(
        &self,
        upstream: &[Rc<dyn CritUpstreamNode<Self::Target>>],
        all: bool,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        if upstream.is_empty() {
            return self.match_constant()[all].clone();
        }
        CritMiddle::new(self, upstream, CritMatchAnyOrAll::new(upstream, all))
    }

    fn exactly(
        &self,
        upstream: &[Rc<dyn CritUpstreamNode<Self::Target>>],
        num: usize,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        if num > upstream.len() {
            return self.match_constant()[false].clone();
        }
        if num == 0 {
            let upstream: Vec<_> = upstream.iter().map(|u| u.not(self)).collect();
            return self.list(&upstream, true);
        }
        CritMiddle::new(self, upstream, CritMatchExactly::new(upstream, num))
    }

    fn leaf(
        &self,
        upstream: &Rc<dyn CritUpstreamNode<Self::Target>>,
        on_match: impl Fn(<Self::Target as CritTarget>::LeafData) -> Box<dyn FnOnce()> + 'static,
    ) -> Rc<CritLeafMatcher<Self::Target>> {
        CritLeafMatcher::new(self, upstream, on_match)
    }

    fn not(
        &self,
        upstream: &Rc<dyn CritUpstreamNode<Self::Target>>,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        upstream.not(self)
    }

    fn root<T>(&self, criterion: T) -> Rc<dyn CritUpstreamNode<Self::Target>>
    where
        T: CritRootCriterion<Self::Target>,
    {
        CritRoot::new(self.roots(), self.id(), criterion)
    }
}

impl<T> CritMgrExt for T where T: CritMgr {}
