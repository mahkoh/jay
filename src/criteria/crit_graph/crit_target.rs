use {
    crate::{
        criteria::{
            CritDestroyListener, CritMatcherId, FixedRootMatcher, crit_leaf::CritLeafEvent,
            crit_matchers::critm_constant::CritMatchConstant,
        },
        utils::{copyhashmap::CopyHashMap, queue::AsyncQueue},
    },
    std::{
        hash::Hash,
        rc::{Rc, Weak},
    },
};

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
