macro_rules! fixed_root_criterion {
    ($ty:ty, $field:ident) => {
        impl crate::criteria::crit_graph::CritFixedRootCriterionBase<crate::tree::ToplevelData>
            for $ty
        {
            fn constant(&self) -> bool {
                self.0
            }

            fn not<'a>(
                &self,
                mgr: &'a crate::criteria::tlm::TlMatcherManager,
            ) -> &'a crate::criteria::FixedRootMatcher<crate::tree::ToplevelData, Self> {
                &mgr.$field
            }
        }
    };
}

pub mod tlmm_client;
pub mod tlmm_floating;
pub mod tlmm_kind;
pub mod tlmm_string;
