use crate::criteria::CritMatcherIds;
use crate::criteria::FixedRootMatcher;
use crate::criteria::crit_graph::CritFixedRootCriterion;
use crate::criteria::crit_graph::CritFixedRootCriterionBase;
use crate::criteria::crit_graph::CritMgr;
use crate::criteria::crit_graph::CritRoot;
use crate::criteria::crit_graph::CritRootFixed;
use crate::criteria::crit_graph::CritTarget;
use linearize::static_map;
use std::marker::PhantomData;
use std::rc::Rc;

pub struct CritMatchConstant<Target>(pub bool, pub PhantomData<fn(&Target)>);

impl<Target> CritMatchConstant<Target>
where
    Target: CritTarget,
{
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
