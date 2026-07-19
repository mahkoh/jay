use crate::criteria::CritDestroyListener;
use crate::criteria::CritMatcherId;
use crate::criteria::FixedRootMatcher;
use crate::criteria::crit_leaf::CritLeafEvent;
use crate::criteria::crit_matchers::critm_constant::CritMatchConstant;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::queue::AsyncQueue;
use std::hash::Hash;
use std::rc::Rc;
use std::rc::Weak;

pub trait CritMgr: 'static {
    type Target: CritTarget<Mgr = Self>;

    fn id(&self) -> CritMatcherId;
    fn leaf_events(&self) -> &Rc<AsyncQueue<CritLeafEvent<Self::Target>>>;
    fn match_constant(&self) -> &FixedRootMatcher<Self::Target, CritMatchConstant<Self::Target>>;
    fn roots(&self) -> &Rc<<Self::Target as CritTarget>::RootMatchers>;
}

pub trait CritTarget: 'static {
    type Id: Copy + Hash + Eq;
    type Mgr: CritMgr<Target = Self>;
    type RootMatchers;
    type LeafData: Copy + Eq;
    type Owner: WeakCritTargetOwner<Target = Self>;

    fn owner(&self) -> Self::Owner;
    fn id(&self) -> Self::Id;
    fn destroyed(&self) -> &CopyHashMap<CritMatcherId, Weak<dyn CritDestroyListener<Self>>>;
    fn leaf_data(&self) -> Self::LeafData;
}

pub trait CritTargetOwner: 'static {
    type Target: CritTarget;

    fn data(&self) -> &Self::Target;
}

pub trait WeakCritTargetOwner: 'static {
    type Target: CritTarget;
    type Owner: CritTargetOwner<Target = Self::Target>;

    fn upgrade(&self) -> Option<Self::Owner>;
}
