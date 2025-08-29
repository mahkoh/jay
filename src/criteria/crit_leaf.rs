use {
    crate::{
        criteria::{
            CritUpstreamNode,
            crit_graph::{CritDownstream, CritDownstreamData, CritMgr, CritTarget},
            crit_per_target_data::{CritDestroyListenerBase, CritPerTargetData},
        },
        utils::{cell_ext::CellExt, queue::AsyncQueue},
    },
    std::{
        cell::Cell,
        rc::{Rc, Weak},
        slice,
    },
};

pub struct CritLeafMatcher<Target>
where
    Target: CritTarget,
{
    upstream: CritDownstreamData<Target>,
    on_match: Box<dyn Fn(Target::LeafData) -> Box<dyn FnOnce()>>,
    targets: CritPerTargetData<Target, NodeHolder<Target>>,
    events: Rc<AsyncQueue<CritLeafEvent<Target>>>,
}

pub(in crate::criteria) struct NodeHolder<Target>
where
    Target: CritTarget,
{
    node: Rc<Node<Target>>,
}

struct Node<Target>
where
    Target: CritTarget,
{
    leaf: Weak<CritLeafMatcher<Target>>,
    target_id: Target::Id,
    needs_event: Cell<bool>,
    new_data: Cell<Option<Target::LeafData>>,
    data: Cell<Option<Target::LeafData>>,
    on_unmatch: Cell<Option<Box<dyn FnOnce()>>>,
}

pub struct CritLeafEvent<Target>
where
    Target: CritTarget,
{
    node: Rc<Node<Target>>,
}

impl<Target> CritDownstream<Target> for CritLeafMatcher<Target>
where
    Target: CritTarget,
{
    fn update_matched(self: Rc<Self>, data: &Target, matched: bool) {
        let node = &self
            .targets
            .get_or_create(data, || {
                let node = Rc::new(Node {
                    leaf: Rc::downgrade(&self),
                    target_id: data.id(),
                    needs_event: Cell::new(true),
                    new_data: Cell::new(None),
                    data: Cell::new(None),
                    on_unmatch: Cell::new(None),
                });
                NodeHolder { node: node.clone() }
            })
            .node;
        self.push_event(node, matched.then_some(data.leaf_data()));
    }
}

impl<Target> CritLeafMatcher<Target>
where
    Target: CritTarget,
{
    pub(in crate::criteria) fn new(
        mgr: &Target::Mgr,
        upstream: &Rc<dyn CritUpstreamNode<Target>>,
        on_match: impl Fn(Target::LeafData) -> Box<dyn FnOnce()> + 'static,
    ) -> Rc<Self> {
        let id = mgr.id();
        let slf = Rc::new_cyclic(|slf| Self {
            targets: CritPerTargetData::new(slf, id),
            on_match: Box::new(on_match),
            events: mgr.leaf_events().clone(),
            upstream: CritDownstreamData::new(id, slice::from_ref(upstream)),
        });
        slf.upstream.attach(&slf);
        slf
    }

    fn push_event(&self, node: &Rc<Node<Target>>, new_data: Option<Target::LeafData>) {
        node.new_data.set(new_data);
        if node.needs_event.replace(false) {
            self.events.push(CritLeafEvent { node: node.clone() });
        }
    }
}

impl<Target> CritLeafEvent<Target>
where
    Target: CritTarget,
{
    pub fn run(self) {
        let n = self.node;
        n.needs_event.set(true);
        if n.new_data != n.data
            && let Some(on_unmatch) = n.on_unmatch.take()
        {
            if n.leaf.strong_count() == 0 {
                return;
            }
            on_unmatch();
        }
        n.data.set(n.new_data.get());
        if n.data.is_some() != n.on_unmatch.is_some() {
            let Some(leaf) = n.leaf.upgrade() else {
                return;
            };
            if let Some(id) = n.data.get() {
                n.on_unmatch.set(Some((leaf.on_match)(id)));
            } else {
                if let Some(on_unmatch) = n.on_unmatch.take() {
                    on_unmatch();
                }
                leaf.targets.remove(n.target_id);
            }
        }
    }
}

impl<Target> Drop for NodeHolder<Target>
where
    Target: CritTarget,
{
    fn drop(&mut self) {
        if let Some(leaf) = self.node.leaf.upgrade() {
            leaf.push_event(&self.node, None);
        }
    }
}

impl<Target> CritDestroyListenerBase<Target> for CritLeafMatcher<Target>
where
    Target: CritTarget,
{
    type Data = NodeHolder<Target>;

    fn data(&self) -> &CritPerTargetData<Target, Self::Data> {
        &self.targets
    }
}
