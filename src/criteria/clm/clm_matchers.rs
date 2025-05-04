#[expect(unused_macros)]
macro_rules! fixed_root_criterion {
    ($ty:ty, $field:ident) => {
        impl crate::criteria::crit_graph::CritFixedRootCriterionBase<Rc<crate::client::Client>>
            for $ty
        {
            fn constant(&self) -> bool {
                self.0
            }

            fn not<'a>(
                &self,
                mgr: &'a crate::criteria::clm::ClMatcherManager,
            ) -> &'a crate::criteria::FixedRootMatcher<Rc<crate::client::Client>, Self> {
                &mgr.$field
            }
        }
    };
}
