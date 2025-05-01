pub mod tlm_matchers;

use {
    crate::{
        criteria::{
            CritDestroyListener, CritLiteralOrRegex, CritMatcherId, CritMatcherIds, CritMgrExt,
            CritUpstreamNode, FixedRootMatcher, RootMatcherMap,
            clm::ClmUpstreamNode,
            crit_graph::{CritMgr, CritTarget, CritTargetOwner, WeakCritTargetOwner},
            crit_leaf::{CritLeafEvent, CritLeafMatcher},
            crit_matchers::critm_constant::CritMatchConstant,
            tlm::tlm_matchers::{
                tlmm_client::TlmMatchClient,
                tlmm_kind::TlmMatchKind,
                tlmm_string::{TlmMatchAppId, TlmMatchTitle},
            },
        },
        state::State,
        tree::{NodeId, ToplevelData, ToplevelNode},
        utils::{
            copyhashmap::CopyHashMap, hash_map_ext::HashMapExt, queue::AsyncQueue,
            toplevel_identifier::ToplevelIdentifier,
        },
    },
    jay_config::window::WindowType,
    std::rc::{Rc, Weak},
};

bitflags! {
    TlMatcherChange: u32;
    TL_CHANGED_DESTROYED   = 1 << 0,
    TL_CHANGED_NEW         = 1 << 1,
    TL_CHANGED_TITLE       = 1 << 2,
    TL_CHANGED_APP_ID      = 1 << 3,
}

type TlmFixedRootMatcher<T> = FixedRootMatcher<ToplevelData, T>;

pub struct TlMatcherManager {
    ids: Rc<CritMatcherIds>,
    changes: AsyncQueue<Rc<dyn ToplevelNode>>,
    leaf_events: Rc<AsyncQueue<CritLeafEvent<ToplevelData>>>,
    constant: TlmFixedRootMatcher<CritMatchConstant<ToplevelData>>,
    matchers: Rc<RootMatchers>,
}

type TlmRootMatcherMap<T> = RootMatcherMap<ToplevelData, T>;

#[derive(Default)]
pub struct RootMatchers {
    kinds: TlmRootMatcherMap<TlmMatchKind>,
    clients: CopyHashMap<CritMatcherId, Weak<TlmMatchClient>>,
    title: TlmRootMatcherMap<TlmMatchTitle>,
    app_id: TlmRootMatcherMap<TlmMatchAppId>,
}

pub async fn handle_tl_changes(state: Rc<State>) {
    let mgr = &state.tl_matcher_manager;
    loop {
        let tl = mgr.changes.pop().await;
        mgr.update_matches(tl);
    }
}

pub async fn handle_tl_leaf_events(state: Rc<State>) {
    let mgr = &state.tl_matcher_manager;
    let debouncer = state.ring.debouncer(1000);
    loop {
        let event = mgr.leaf_events.pop().await;
        event.run();
        debouncer.debounce().await;
    }
}

pub type TlmUpstreamNode = dyn CritUpstreamNode<ToplevelData>;
pub type TlmLeafMatcher = CritLeafMatcher<ToplevelData>;

impl TlMatcherManager {
    pub fn new(ids: &Rc<CritMatcherIds>) -> Self {
        let matchers = Rc::new(RootMatchers::default());
        Self {
            constant: CritMatchConstant::create(&matchers, ids),
            changes: Default::default(),
            leaf_events: Default::default(),
            ids: ids.clone(),
            matchers,
        }
    }

    pub fn clear(&self) {
        self.changes.clear();
        self.leaf_events.clear();
    }

    pub fn rematch_all(&self, state: &Rc<State>) {
        for tl in state.toplevels.lock().values() {
            if let Some(tl) = tl.upgrade() {
                tl.tl_data().property_changed(TL_CHANGED_NEW);
            }
        }
    }

    pub fn has_no_interest(&self, data: &ToplevelData, change: TlMatcherChange) -> bool {
        !self.has_interest(data, change)
    }

    pub fn has_interest(&self, data: &ToplevelData, mut change: TlMatcherChange) -> bool {
        if change.contains(TL_CHANGED_DESTROYED) && data.destroyed.is_not_empty() {
            return true;
        }
        #[expect(unused_macros)]
        macro_rules! fixed {
            ($name:ident) => {
                if self.$name[false].has_downstream() || self.$name[true].has_downstream() {
                    return true;
                }
            };
        }
        if change.contains(TL_CHANGED_NEW) {
            macro_rules! unconditional {
                ($field:ident) => {
                    if self.matchers.$field.is_not_empty() {
                        return true;
                    }
                };
            }
            unconditional!(kinds);
            unconditional!(clients);
            if self.constant[true].has_downstream() {
                return true;
            }
            change |= TlMatcherChange::all();
        }
        macro_rules! conditional {
            ($change:expr, $field:ident) => {
                if change.contains($change) && self.matchers.$field.is_not_empty() {
                    return true;
                }
            };
        }
        #[expect(unused_macros)]
        macro_rules! fixed_conditional {
            ($change:expr, $field:ident) => {
                if change.contains($change) {
                    fixed!($field);
                }
            };
        }
        conditional!(TL_CHANGED_TITLE, title);
        conditional!(TL_CHANGED_APP_ID, app_id);
        false
    }

    pub fn changed(&self, node: Rc<dyn ToplevelNode>) {
        self.changes.push(node);
    }

    fn update_matches(&self, node: Rc<dyn ToplevelNode>) {
        let data = node.tl_data();
        let mut changed = data.changed_properties.replace(TlMatcherChange::none());
        if changed.contains(TL_CHANGED_DESTROYED) {
            for destroyed in data.destroyed.lock().drain_values() {
                if let Some(destroyed) = destroyed.upgrade() {
                    destroyed.destroyed(data.node_id);
                }
            }
        }
        if data.parent.is_none() {
            return;
        }
        macro_rules! handlers {
            ($name:ident) => {
                self.matchers
                    .$name
                    .lock()
                    .values()
                    .filter_map(|m| m.upgrade())
            };
        }
        #[expect(unused_macros)]
        macro_rules! fixed {
            ($name:ident) => {
                self.$name[false].handle(data);
                self.$name[true].handle(data);
            };
        }
        if changed.contains(TL_CHANGED_NEW) {
            changed |= TlMatcherChange::all();
            macro_rules! unconditional {
                ($field:ident) => {
                    for m in handlers!($field) {
                        m.handle(data);
                    }
                };
            }
            unconditional!(kinds);
            unconditional!(clients);
            self.constant[true].handle(data);
        }
        macro_rules! conditional {
            ($change:expr, $field:ident) => {
                if changed.contains($change) {
                    for m in handlers!($field) {
                        m.handle(data);
                    }
                }
            };
        }
        #[expect(unused_macros)]
        macro_rules! fixed_conditional {
            ($change:expr, $field:ident) => {
                if changed.contains($change) {
                    fixed!($field);
                }
            };
        }
        conditional!(TL_CHANGED_TITLE, title);
        conditional!(TL_CHANGED_APP_ID, app_id);
    }

    pub fn title(&self, string: CritLiteralOrRegex) -> Rc<TlmUpstreamNode> {
        self.root(TlmMatchTitle::new(string))
    }

    pub fn app_id(&self, string: CritLiteralOrRegex) -> Rc<TlmUpstreamNode> {
        self.root(TlmMatchAppId::new(string))
    }

    pub fn kind(&self, kind: WindowType) -> Rc<TlmUpstreamNode> {
        self.root(TlmMatchKind::new(kind))
    }

    pub fn client(&self, state: &Rc<State>, client: &Rc<ClmUpstreamNode>) -> Rc<TlmUpstreamNode> {
        TlmMatchClient::new(state, client)
    }
}

impl CritTarget for ToplevelData {
    type Id = NodeId;
    type Mgr = TlMatcherManager;
    type RootMatchers = RootMatchers;
    type LeafData = ToplevelIdentifier;
    type Owner = Weak<dyn ToplevelNode>;

    fn owner(&self) -> Self::Owner {
        self.slf.clone()
    }

    fn id(&self) -> Self::Id {
        self.node_id
    }

    fn destroyed(&self) -> &CopyHashMap<CritMatcherId, Weak<dyn CritDestroyListener<Self>>> {
        &self.destroyed
    }

    fn leaf_data(&self) -> Self::LeafData {
        self.identifier.get()
    }
}

impl CritTargetOwner for Rc<dyn ToplevelNode> {
    type Target = ToplevelData;

    fn data(&self) -> &Self::Target {
        self.tl_data()
    }
}

impl WeakCritTargetOwner for Weak<dyn ToplevelNode> {
    type Target = ToplevelData;
    type Owner = Rc<dyn ToplevelNode>;

    fn upgrade(&self) -> Option<Self::Owner> {
        self.upgrade()
    }
}

impl CritMgr for TlMatcherManager {
    type Target = ToplevelData;

    fn id(&self) -> CritMatcherId {
        self.ids.next()
    }

    fn leaf_events(&self) -> &Rc<AsyncQueue<CritLeafEvent<Self::Target>>> {
        &self.leaf_events
    }

    fn match_constant(&self) -> &FixedRootMatcher<Self::Target, CritMatchConstant<Self::Target>> {
        &self.constant
    }

    fn roots(&self) -> &Rc<<Self::Target as CritTarget>::RootMatchers> {
        &self.matchers
    }
}
