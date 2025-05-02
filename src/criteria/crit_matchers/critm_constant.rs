use {
    crate::criteria::{
        CritMatcherIds, FixedRootMatcher,
        crit_graph::{
            CritFixedRootCriterion, CritFixedRootCriterionBase, CritMgr, CritRoot, CritRootFixed,
            CritTarget,
        },
    },
    linearize::static_map,
    std::{marker::PhantomData, rc::Rc},
};

pub struct CritMatchConstant<Target>(pub bool, pub PhantomData<fn(&Target)>);

impl<Target> CritMatchConstant<Target>
where
    Target: CritTarget,
{
    #[expect(dead_code)]
    pub fn create(
        roots: &Rc<Target::RootMatchers>,
        ids: &CritMatcherIds,
    ) -> FixedRootMatcher<Target, CritMatchConstant<Target>> {
        static_map! {
            v => CritRoot::new(
                roots,
                ids.next(),
                CritRootFixed(Self(v, PhantomData), PhantomData),
            ),
        }
    }
}

impl<Target> CritFixedRootCriterionBase<Target> for CritMatchConstant<Target>
where
    Target: CritTarget,
{
    fn constant(&self) -> bool {
        self.0
    }

    fn not<'a>(&self, mgr: &'a Target::Mgr) -> &'a FixedRootMatcher<Target, Self>
    where
        Self: CritFixedRootCriterion<Target>,
    {
        mgr.match_constant()
    }
}

impl<Target> CritFixedRootCriterion<Target> for CritMatchConstant<Target>
where
    Target: CritTarget,
{
    const COMPARE: bool = false;

    fn matches(&self, _data: &Target) -> bool {
        self.0
    }
}
