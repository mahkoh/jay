use {
    crate::{
        client::Client,
        criteria::{
            CritMatcherId, CritUpstreamNode,
            clm::ClmUpstreamNode,
            crit_graph::{
                CritDownstream, CritDownstreamData, CritMgr, CritUpstreamData,
                CritUpstreamNodeBase, CritUpstreamNodeData,
            },
            crit_per_target_data::{CritDestroyListenerBase, CritPerTargetData},
            tlm::TlMatcherManager,
        },
        state::State,
        tree::{ToplevelData, ToplevelNodeBase},
    },
    std::rc::Rc,
};

pub struct TlmMatchClient {
    id: CritMatcherId,
    state: Rc<State>,
    node: Rc<ClmUpstreamNode>,
    upstream: CritDownstreamData<Rc<Client>>,
    downstream: CritUpstreamData<ToplevelData, ()>,
}

impl TlmMatchClient {
    pub fn new(state: &Rc<State>, node: &Rc<ClmUpstreamNode>) -> Rc<TlmMatchClient> {
        let id = state.tl_matcher_manager.id();
        let slf = Rc::new_cyclic(|slf| Self {
            id,
            state: state.clone(),
            node: node.clone(),
            upstream: CritDownstreamData::new(id, &[node.clone()]),
            downstream: CritUpstreamData::new(slf, id),
        });
        slf.upstream.attach(&slf);
        state
            .tl_matcher_manager
            .matchers
            .clients
            .set(id, Rc::downgrade(&slf));
        slf
    }

    pub fn handle(&self, node: &ToplevelData) {
        if let Some(client) = &node.client
            && self.node.get(client)
        {
            let data = self.downstream.get_or_create(node);
            self.downstream.update_matched(node, data, true, false);
        }
    }
}

impl CritUpstreamNodeBase<ToplevelData> for TlmMatchClient {
    type Data = ();

    fn data(&self) -> &CritUpstreamData<ToplevelData, Self::Data> {
        &self.downstream
    }

    fn not(&self, _mgr: &TlMatcherManager) -> Rc<dyn CritUpstreamNode<ToplevelData>> {
        Self::new(&self.state, &self.node.not(&self.state.cl_matcher_manager))
    }

    fn pull(&self, target: &ToplevelData) -> bool {
        if let Some(client) = &target.client {
            return self.node.pull(client);
        }
        false
    }
}

impl CritDownstream<Rc<Client>> for TlmMatchClient {
    fn update_matched(self: Rc<Self>, target: &Rc<Client>, matched: bool) {
        let handle = |data: &ToplevelData| {
            let node = match matched {
                true => self.downstream.get_or_create(data),
                false => match self.downstream.get(data) {
                    Some(n) => n,
                    None => return,
                },
            };
            self.downstream
                .update_matched(data, node, matched, !matched);
        };
        if target.is_xwayland {
            for tl in self.state.xwayland.windows.lock().values() {
                handle(tl.tl_data());
            }
        } else {
            for tl in target.objects.xdg_toplevel.lock().values() {
                handle(tl.tl_data());
            }
        }
    }
}

impl CritDestroyListenerBase<ToplevelData> for TlmMatchClient {
    type Data = CritUpstreamNodeData<ToplevelData, ()>;

    fn data(&self) -> &CritPerTargetData<ToplevelData, Self::Data> {
        &self.downstream.nodes
    }
}

impl Drop for TlmMatchClient {
    fn drop(&mut self) {
        self.state
            .tl_matcher_manager
            .matchers
            .clients
            .remove(&self.id);
    }
}
