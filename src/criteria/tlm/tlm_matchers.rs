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
pub mod tlmm_content_type;
pub mod tlmm_floating;
pub mod tlmm_fullscreen;
pub mod tlmm_just_mapped;
pub mod tlmm_kind;
pub mod tlmm_seat_focus;
pub mod tlmm_string;
pub mod tlmm_urgent;
pub mod tlmm_visible;
