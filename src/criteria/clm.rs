pub mod clm_matchers;

use crate::client::Client;
use crate::client::ClientId;
use crate::criteria::CritDestroyListener;
use crate::criteria::CritLiteralOrRegex;
use crate::criteria::CritMatcherId;
use crate::criteria::CritMatcherIds;
use crate::criteria::CritMgrExt;
use crate::criteria::CritUpstreamNode;
use crate::criteria::FixedRootMatcher;
use crate::criteria::RootMatcherMap;
use crate::criteria::clm::clm_matchers::clmm_id::ClmMatchId;
use crate::criteria::clm::clm_matchers::clmm_is_xwayland::ClmMatchIsXwayland;
use crate::criteria::clm::clm_matchers::clmm_pid::ClmMatchPid;
use crate::criteria::clm::clm_matchers::clmm_sandboxed::ClmMatchSandboxed;
use crate::criteria::clm::clm_matchers::clmm_string::ClmMatchComm;
use crate::criteria::clm::clm_matchers::clmm_string::ClmMatchExe;
use crate::criteria::clm::clm_matchers::clmm_string::ClmMatchSandboxAppId;
use crate::criteria::clm::clm_matchers::clmm_string::ClmMatchSandboxEngine;
use crate::criteria::clm::clm_matchers::clmm_string::ClmMatchSandboxInstanceId;
use crate::criteria::clm::clm_matchers::clmm_string::ClmMatchTag;
use crate::criteria::clm::clm_matchers::clmm_uid::ClmMatchUid;
use crate::criteria::crit_graph::CritMgr;
use crate::criteria::crit_graph::CritRoot;
use crate::criteria::crit_graph::CritRootFixed;
use crate::criteria::crit_graph::CritTarget;
use crate::criteria::crit_graph::CritTargetOwner;
use crate::criteria::crit_graph::WeakCritTargetOwner;
use crate::criteria::crit_leaf::CritLeafEvent;
use crate::criteria::crit_leaf::CritLeafMatcher;
use crate::criteria::crit_matchers::critm_constant::CritMatchConstant;
use crate::state::State;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::hash_map_ext::HashMapExt;
use crate::utils::queue::AsyncQueue;
use linearize::static_map;
use std::marker::PhantomData;
use std::rc::Rc;
use std::rc::Weak;

bitflags! {
    ClMatcherChange: u32;
    CL_CHANGED_DESTROYED,
    CL_CHANGED_NEW,
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
    tag: ClmRootMatcherMap<ClmMatchTag>,
    id: ClmRootMatcherMap<ClmMatchId>,
}

impl RootMatchers {
    fn clear(&self) {
        self.sandbox_app_id.clear();
        self.sandbox_engine.clear();
        self.sandbox_instance_id.clear();
        self.uid.clear();
        self.pid.clear();
        self.comm.clear();
        self.exe.clear();
        self.tag.clear();
        self.id.clear();
    }
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
        self.constant.values().for_each(|c| c.clear());
        self.sandboxed.values().for_each(|c| c.clear());
        self.is_xwayland.values().for_each(|c| c.clear());
        self.matchers.clear();
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
            unconditional!(tag);
            unconditional!(id);
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

    pub fn tag(&self, string: CritLiteralOrRegex) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchTag::new(string))
    }

    pub fn id(&self, id: ClientId) -> Rc<ClmUpstreamNode> {
        self.root(ClmMatchId(id))
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
