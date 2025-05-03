pub mod clm_matchers;

use {
    crate::{
        client::{Client, ClientId},
        criteria::{
            CritDestroyListener, CritLiteralOrRegex, CritMatcherId, CritMatcherIds, CritMgrExt,
            CritUpstreamNode, FixedRootMatcher, RootMatcherMap,
            clm::clm_matchers::{
                clmm_is_xwayland::ClmMatchIsXwayland,
                clmm_pid::ClmMatchPid,
                clmm_sandboxed::ClmMatchSandboxed,
                clmm_string::{
                    ClmMatchComm, ClmMatchExe, ClmMatchSandboxAppId, ClmMatchSandboxEngine,
                    ClmMatchSandboxInstanceId,
                },
                clmm_uid::ClmMatchUid,
            },
            crit_graph::{
                CritMgr, CritRoot, CritRootFixed, CritTarget, CritTargetOwner, WeakCritTargetOwner,
            },
            crit_leaf::{CritLeafEvent, CritLeafMatcher},
            crit_matchers::critm_constant::CritMatchConstant,
        },
        state::State,
        utils::{copyhashmap::CopyHashMap, hash_map_ext::HashMapExt, queue::AsyncQueue},
    },
    linearize::static_map,
    std::{
        marker::PhantomData,
        rc::{Rc, Weak},
    },
};

bitflags! {
    ClMatcherChange: u32;
    CL_CHANGED_DESTROYED   = 1 << 0,
    CL_CHANGED_NEW         = 1 << 1,
}

type ClmFixedRootMatcher<T> = FixedRootMatcher<Rc<Client>, T>;

pub struct ClMatcherManager {
    ids: Rc<CritMatcherIds>,
    changes: AsyncQueue<Rc<Client>>,
    leaf_events: Rc<AsyncQueue<CritLeafEvent<Rc<Client>>>>,
    constant: ClmFixedRootMatcher<CritMatchConstant<Rc<Client>>>,
    sandboxed: ClmFixedRootMatcher<ClmMatchSandboxed>,
    is_xwayland: ClmFixedRootMatcher<ClmMatchIsXwayland>,
    matchers: Rc<RootMatchers>,
}

type ClmRootMatcherMap<T> = RootMatcherMap<Rc<Client>, T>;

#[derive(Default)]
pub struct RootMatchers {
    sandbox_app_id: ClmRootMatcherMap<ClmMatchSandboxAppId>,
    sandbox_engine: ClmRootMatcherMap<ClmMatchSandboxEngine>,
    sandbox_instance_id: ClmRootMatcherMap<ClmMatchSandboxInstanceId>,
    uid: ClmRootMatcherMap<ClmMatchUid>,
    pid: ClmRootMatcherMap<ClmMatchPid>,
    comm: ClmRootMatcherMap<ClmMatchComm>,
    exe: ClmRootMatcherMap<ClmMatchExe>,
}

pub async fn handle_cl_changes(state: Rc<State>) {
    let mgr = &state.cl_matcher_manager;
    loop {
        let tl = mgr.changes.pop().await;
        mgr.update_matches(&tl);
    }
}

pub async fn handle_cl_leaf_events(state: Rc<State>) {
    let mgr = &state.cl_matcher_manager;
    let debouncer = state.ring.debouncer(1000);
    loop {
        let event = mgr.leaf_events.pop().await;
        event.run();
        debouncer.debounce().await;
    }
}

pub type ClmUpstreamNode = dyn CritUpstreamNode<Rc<Client>>;
pub type ClmLeafMatcher = CritLeafMatcher<Rc<Client>>;

impl ClMatcherManager {
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
            sandboxed: bool!(ClmMatchSandboxed),
            is_xwayland: bool!(ClmMatchIsXwayland),
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
        for client in state.clients.clients.borrow().values() {
            client.data.property_changed(CL_CHANGED_NEW);
        }
    }

    pub fn changed(&self, client: &Rc<Client>) {
        self.changes.push(client.clone());
    }

    fn update_matches(&self, data: &Rc<Client>) {
        let mut changed = data.changed_properties.take();
        if changed.contains(CL_CHANGED_DESTROYED) {
            for destroyed in data.destroyed.lock().drain_values() {
                if let Some(destroyed) = destroyed.upgrade() {
                    destroyed.destroyed(data.id);
                }
            }
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
        if changed.contains(CL_CHANGED_NEW) {
            changed |= ClMatcherChange::all();
            macro_rules! unconditional {
                ($field:ident) => {
                    for m in handlers!($field) {
                        m.handle(data);
                    }
                };
            }
            unconditional!(sandbox_instance_id);
            unconditional!(sandbox_app_id);
            unconditional!(sandbox_engine);
            unconditional!(uid);
            unconditional!(pid);
            unconditional!(comm);
            unconditional!(exe);
            fixed!(sandboxed);
            fixed!(is_xwayland);
            self.constant[true].handle(data);
        }
    }

    pub fn sandbox_engine(&self, string: CritLiteralOrRegex) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchSandboxEngine::new(string))
    }

    pub fn sandbox_app_id(&self, string: CritLiteralOrRegex) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchSandboxAppId::new(string))
    }

    pub fn sandbox_instance_id(&self, string: CritLiteralOrRegex) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchSandboxInstanceId::new(string))
    }

    pub fn sandboxed(&self) -> Rc<ClmUpstreamNode> {
        self.sandboxed[true].clone()
    }

    pub fn uid(&self, pid: i32) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchUid(pid as _))
    }

    pub fn pid(&self, pid: i32) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchPid(pid as _))
    }

    pub fn is_xwayland(&self) -> Rc<ClmUpstreamNode> {
        self.is_xwayland[true].clone()
    }

    pub fn comm(&self, string: CritLiteralOrRegex) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchComm::new(string))
    }

    pub fn exe(&self, string: CritLiteralOrRegex) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchExe::new(string))
    }
}

impl CritTarget for Rc<Client> {
    type Id = ClientId;
    type Mgr = ClMatcherManager;
    type RootMatchers = RootMatchers;
    type LeafData = ClientId;
    type Owner = Weak<Client>;

    fn owner(&self) -> Self::Owner {
        Rc::downgrade(self)
    }

    fn id(&self) -> Self::Id {
        self.id
    }

    fn destroyed(&self) -> &CopyHashMap<CritMatcherId, Weak<dyn CritDestroyListener<Self>>> {
        &self.destroyed
    }

    fn leaf_data(&self) -> Self::LeafData {
        self.id
    }
}

impl CritTargetOwner for Rc<Client> {
    type Target = Rc<Client>;

    fn data(&self) -> &Self::Target {
        self
    }
}

impl WeakCritTargetOwner for Weak<Client> {
    type Target = Rc<Client>;
    type Owner = Rc<Client>;

    fn upgrade(&self) -> Option<Self::Owner> {
        self.upgrade()
    }
}

impl CritMgr for ClMatcherManager {
    type Target = Rc<Client>;

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
