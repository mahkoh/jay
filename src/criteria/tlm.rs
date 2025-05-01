pub mod tlm_matchers;

use {
    crate::{
        criteria::{
            CritDestroyListener, CritLiteralOrRegex, CritMatcherId, CritMatcherIds, CritMgrExt,
            CritUpstreamNode, FixedRootMatcher, RootMatcherMap,
            clm::ClmUpstreamNode,
            crit_graph::{
                CritMgr, CritRoot, CritRootFixed, CritTarget, CritTargetOwner, WeakCritTargetOwner,
            },
            crit_leaf::{CritLeafEvent, CritLeafMatcher},
            crit_matchers::critm_constant::CritMatchConstant,
            tlm::tlm_matchers::{
                tlmm_client::TlmMatchClient,
                tlmm_floating::TlmMatchFloating,
                tlmm_fullscreen::TlmMatchFullscreen,
                tlmm_just_mapped::TlmMatchJustMapped,
                tlmm_kind::TlmMatchKind,
                tlmm_seat_focus::TlmMatchSeatFocus,
                tlmm_string::{TlmMatchAppId, TlmMatchTag, TlmMatchTitle},
                tlmm_urgent::TlmMatchUrgent,
                tlmm_visible::TlmMatchVisible,
            },
        },
        ifs::wl_seat::WlSeatGlobal,
        state::State,
        tree::{NodeId, ToplevelData, ToplevelNode},
        utils::{
            copyhashmap::CopyHashMap, hash_map_ext::HashMapExt, queue::AsyncQueue,
            toplevel_identifier::ToplevelIdentifier,
        },
    },
    jay_config::window::WindowType,
    linearize::static_map,
    std::{
        marker::PhantomData,
        rc::{Rc, Weak},
    },
};

bitflags! {
    TlMatcherChange: u32;
    TL_CHANGED_DESTROYED   = 1 << 0,
    TL_CHANGED_NEW         = 1 << 1,
    TL_CHANGED_TITLE       = 1 << 2,
    TL_CHANGED_APP_ID      = 1 << 3,
    TL_CHANGED_FLOATING    = 1 << 4,
    TL_CHANGED_VISIBLE     = 1 << 5,
    TL_CHANGED_URGENT      = 1 << 6,
    TL_CHANGED_SEAT_FOCI   = 1 << 7,
    TL_CHANGED_FULLSCREEN  = 1 << 8,
    TL_CHANGED_JUST_MAPPED = 1 << 9,
    TL_CHANGED_TAG         = 1 << 10,
}

type TlmFixedRootMatcher<T> = FixedRootMatcher<ToplevelData, T>;

pub struct TlMatcherManager {
    ids: Rc<CritMatcherIds>,
    changes: AsyncQueue<Rc<dyn ToplevelNode>>,
    leaf_events: Rc<AsyncQueue<CritLeafEvent<ToplevelData>>>,
    handle_just_mapped: AsyncQueue<Rc<dyn ToplevelNode>>,
    constant: TlmFixedRootMatcher<CritMatchConstant<ToplevelData>>,
    floating: TlmFixedRootMatcher<TlmMatchFloating>,
    visible: TlmFixedRootMatcher<TlmMatchVisible>,
    urgent: TlmFixedRootMatcher<TlmMatchUrgent>,
    fullscreen: TlmFixedRootMatcher<TlmMatchFullscreen>,
    just_mapped: TlmFixedRootMatcher<TlmMatchJustMapped>,
    matchers: Rc<RootMatchers>,
}

type TlmRootMatcherMap<T> = RootMatcherMap<ToplevelData, T>;

#[derive(Default)]
pub struct RootMatchers {
    kinds: TlmRootMatcherMap<TlmMatchKind>,
    clients: CopyHashMap<CritMatcherId, Weak<TlmMatchClient>>,
    title: TlmRootMatcherMap<TlmMatchTitle>,
    tag: TlmRootMatcherMap<TlmMatchTag>,
    app_id: TlmRootMatcherMap<TlmMatchAppId>,
    seat_foci: TlmRootMatcherMap<TlmMatchSeatFocus>,
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

pub async fn handle_tl_just_mapped(state: Rc<State>) {
    let mgr = &state.tl_matcher_manager;
    loop {
        let tl = mgr.handle_just_mapped.pop().await;
        let data = tl.tl_data();
        data.just_mapped_scheduled.set(false);
        data.property_changed(TL_CHANGED_JUST_MAPPED);
    }
}

pub type TlmUpstreamNode = dyn CritUpstreamNode<ToplevelData>;
pub type TlmLeafMatcher = CritLeafMatcher<ToplevelData>;

impl TlMatcherManager {
    pub fn new(ids: &Rc<CritMatcherIds>) -> Self {
        let matchers = Rc::new(RootMatchers::default());
        macro_rules! bool {
            ($name:ident) => {{
                static_map! {
                    v => CritRoot::new(
                        &matchers,
                        ids.next(),
                        CritRootFixed($name(v), PhantomData),
                    )
                }
            }};
        }
        Self {
            constant: CritMatchConstant::create(&matchers, ids),
            floating: bool!(TlmMatchFloating),
            visible: bool!(TlmMatchVisible),
            urgent: bool!(TlmMatchUrgent),
            fullscreen: bool!(TlmMatchFullscreen),
            just_mapped: bool!(TlmMatchJustMapped),
            changes: Default::default(),
            leaf_events: Default::default(),
            ids: ids.clone(),
            matchers,
            handle_just_mapped: Default::default(),
        }
    }

    pub fn clear(&self) {
        self.changes.clear();
        self.leaf_events.clear();
        self.handle_just_mapped.clear();
    }

    pub fn rematch_all(&self, state: &Rc<State>) {
        for tl in state.toplevels.lock().values() {
            if let Some(tl) = tl.upgrade() {
                tl.tl_data().property_changed(TL_CHANGED_NEW);
            }
        }
    }

    pub fn has_seat_foci(&self) -> bool {
        self.matchers.seat_foci.is_not_empty()
    }

    pub fn has_no_interest(&self, data: &ToplevelData, change: TlMatcherChange) -> bool {
        !self.has_interest(data, change)
    }

    pub fn has_interest(&self, data: &ToplevelData, mut change: TlMatcherChange) -> bool {
        if change.contains(TL_CHANGED_DESTROYED) && data.destroyed.is_not_empty() {
            return true;
        }
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
        macro_rules! fixed_conditional {
            ($change:expr, $field:ident) => {
                if change.contains($change) {
                    fixed!($field);
                }
            };
        }
        conditional!(TL_CHANGED_TITLE, title);
        conditional!(TL_CHANGED_APP_ID, app_id);
        conditional!(TL_CHANGED_SEAT_FOCI, seat_foci);
        conditional!(TL_CHANGED_TAG, tag);
        fixed_conditional!(TL_CHANGED_FLOATING, floating);
        fixed_conditional!(TL_CHANGED_VISIBLE, visible);
        fixed_conditional!(TL_CHANGED_URGENT, urgent);
        fixed_conditional!(TL_CHANGED_FULLSCREEN, fullscreen);
        fixed_conditional!(TL_CHANGED_JUST_MAPPED, just_mapped);
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
        macro_rules! fixed_conditional {
            ($change:expr, $field:ident) => {
                if changed.contains($change) {
                    fixed!($field);
                }
            };
        }
        conditional!(TL_CHANGED_TITLE, title);
        conditional!(TL_CHANGED_APP_ID, app_id);
        conditional!(TL_CHANGED_SEAT_FOCI, seat_foci);
        conditional!(TL_CHANGED_TAG, tag);
        fixed_conditional!(TL_CHANGED_FLOATING, floating);
        fixed_conditional!(TL_CHANGED_VISIBLE, visible);
        fixed_conditional!(TL_CHANGED_URGENT, urgent);
        fixed_conditional!(TL_CHANGED_FULLSCREEN, fullscreen);
        fixed_conditional!(TL_CHANGED_JUST_MAPPED, just_mapped);
        if changed.contains(TL_CHANGED_JUST_MAPPED)
            && data.just_mapped()
            && (self.just_mapped[false].has_downstream() || self.just_mapped[true].has_downstream())
            && !data.just_mapped_scheduled.replace(true)
        {
            self.handle_just_mapped.push(node);
        }
    }

    pub fn title(&self, string: CritLiteralOrRegex) -> Rc<TlmUpstreamNode> {
        self.root(TlmMatchTitle::new(string))
    }

    pub fn app_id(&self, string: CritLiteralOrRegex) -> Rc<TlmUpstreamNode> {
        self.root(TlmMatchAppId::new(string))
    }

    pub fn tag(&self, string: CritLiteralOrRegex) -> Rc<TlmUpstreamNode> {
        self.root(TlmMatchTag::new(string))
    }

    pub fn floating(&self) -> Rc<TlmUpstreamNode> {
        self.floating[true].clone()
    }

    pub fn kind(&self, kind: WindowType) -> Rc<TlmUpstreamNode> {
        self.root(TlmMatchKind::new(kind))
    }

    pub fn client(&self, state: &Rc<State>, client: &Rc<ClmUpstreamNode>) -> Rc<TlmUpstreamNode> {
        TlmMatchClient::new(state, client)
    }

    pub fn visible(&self) -> Rc<TlmUpstreamNode> {
        self.visible[true].clone()
    }

    pub fn fullscreen(&self) -> Rc<TlmUpstreamNode> {
        self.fullscreen[true].clone()
    }

    pub fn urgent(&self) -> Rc<TlmUpstreamNode> {
        self.urgent[true].clone()
    }

    pub fn just_mapped(&self) -> Rc<TlmUpstreamNode> {
        self.just_mapped[true].clone()
    }

    pub fn seat_focus(&self, seat: &WlSeatGlobal) -> Rc<TlmUpstreamNode> {
        self.root(TlmMatchSeatFocus::new(seat.id()))
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
