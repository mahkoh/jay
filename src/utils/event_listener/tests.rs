use crate::utils::event_listener::CACHE;
use crate::utils::event_listener::EventListener;
use crate::utils::event_listener::EventSource;
use crate::utils::event_listener::MAX_CACHED;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::rc::Weak;

fn cached() -> usize {
    CACHE.get().num_heads
}

trait Listener {
    fn notify(&self);
}

type Action = Box<dyn FnMut(&Node, &Rc<World>)>;

struct World {
    src: EventSource<dyn Listener>,
    alt: EventSource<dyn Listener>,
    nodes: RefCell<HashMap<u32, Rc<Node>>>,
    log: RefCell<Vec<String>>,
    calls: Cell<u32>,
}

impl World {
    fn new() -> Rc<Self> {
        Rc::new(World {
            src: Default::default(),
            alt: Default::default(),
            nodes: Default::default(),
            log: Default::default(),
            calls: Default::default(),
        })
    }

    fn add(self: &Rc<Self>, id: u32, action: Option<Action>) -> Rc<Node> {
        let node = Node::new(id, self, action);
        node.listener.attach(&self.src);
        self.nodes.borrow_mut().insert(id, node.clone());
        node
    }

    fn dispatch(self: &Rc<Self>) {
        self.calls.set(0);
        let slf = self.clone();
        self.src.for_each(|listener| {
            let calls = slf.calls.get() + 1;
            slf.calls.set(calls);
            assert!(calls < 10_000, "runaway iteration");
            listener.notify();
        });
    }

    fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

struct Node {
    id: u32,
    world: Weak<World>,
    listener: EventListener<dyn Listener>,
    action: RefCell<Option<Action>>,
}

impl Node {
    fn new(id: u32, world: &Rc<World>, action: Option<Action>) -> Rc<Self> {
        Rc::new_cyclic(|slf: &Weak<Node>| Node {
            id,
            world: Rc::downgrade(world),
            listener: EventListener::new(slf.clone() as Weak<dyn Listener>),
            action: RefCell::new(action),
        })
    }
}

impl Listener for Node {
    fn notify(&self) {
        let world = self.world.upgrade().unwrap();
        world.log.borrow_mut().push(format!("n{}", self.id));
        let mut action = self.action.borrow_mut().take();
        if let Some(action) = &mut action {
            action(self, &world);
        }
        *self.action.borrow_mut() = action;
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        if let Some(world) = self.world.upgrade() {
            world.log.borrow_mut().push(format!("drop{}", self.id));
        }
    }
}

struct Simple;

impl Listener for Simple {
    fn notify(&self) {}
}

#[test]
fn dispatch_order_is_lifo() {
    let world = World::new();
    let _n1 = world.add(1, None);
    let _n2 = world.add(2, None);
    let _n3 = world.add(3, None);
    assert!(world.src.has_listeners());
    world.dispatch();
    assert_eq!(world.log(), ["n3", "n2", "n1"]);
}

#[test]
fn listener_drops_itself_during_dispatch() {
    let world = World::new();
    let _n1 = world.add(1, None);
    world.add(
        2,
        Some(Box::new(|node, world| {
            world.nodes.borrow_mut().remove(&node.id);
        })),
    );
    let _n3 = world.add(3, None);
    world.dispatch();
    assert_eq!(world.log(), ["n3", "n2", "drop2", "n1"]);
    world.dispatch();
    assert_eq!(world.log()[4..], ["n3", "n1"]);
}

#[test]
fn listener_drops_other_during_dispatch() {
    let world = World::new();
    world.add(1, None);
    world.add(2, None);
    world.add(
        3,
        Some(Box::new(|_node, world| {
            world.nodes.borrow_mut().remove(&1);
        })),
    );
    world.dispatch();
    assert_eq!(world.log(), ["n3", "drop1", "n2"]);
}

#[test]
fn detach_and_attach_during_dispatch() {
    let world = World::new();
    let _n1 = world.add(1, Some(Box::new(|node, _world| node.listener.detach())));
    let _n2 = world.add(
        2,
        Some(Box::new(|node, world| node.listener.attach(&world.alt))),
    );
    let _n3 = world.add(3, None);
    world.dispatch();
    assert_eq!(world.log(), ["n3", "n2", "n1"]);
    world.dispatch();
    assert_eq!(world.log()[3..], ["n3"]);
    world.alt.for_each(|listener| listener.notify());
    assert_eq!(world.log()[4..], ["n2"]);
}

#[test]
fn reattach_to_same_source_during_dispatch() {
    let world = World::new();
    let _n1 = world.add(1, None);
    let _n2 = world.add(
        2,
        Some(Box::new(|node, world| node.listener.attach(&world.src))),
    );
    world.dispatch();
    assert_eq!(world.log(), ["n2", "n1"]);
    world.dispatch();
    assert_eq!(world.log()[2..], ["n2", "n1"]);
}

#[test]
fn nested_dispatch() {
    let world = World::new();
    let _n1 = world.add(1, None);
    world.add(
        2,
        Some(Box::new(|node, world| {
            let id = node.id;
            let inner = world.clone();
            world.src.for_each(|_listener| {
                inner.log.borrow_mut().push("inner".to_string());
            });
            world.nodes.borrow_mut().remove(&id);
        })),
    );
    world.dispatch();
    assert_eq!(world.log(), ["n2", "inner", "inner", "drop2", "n1"]);
}

#[test]
fn on_attach_fires_once() {
    let world = World::new();
    let fired = Rc::new(Cell::new(0));
    let fired2 = fired.clone();
    world
        .alt
        .on_attach(Box::new(move || fired2.set(fired2.get() + 1)));
    let n1 = world.add(1, None);
    assert_eq!(fired.get(), 0);
    n1.listener.attach(&world.alt);
    assert_eq!(fired.get(), 1);
    let n2 = world.add(2, None);
    n2.listener.attach(&world.alt);
    assert_eq!(fired.get(), 1);
}

#[test]
fn source_is_empty_after_dispatch_drop() {
    let world = World::new();
    world.add(
        1,
        Some(Box::new(|node, world| {
            world.nodes.borrow_mut().remove(&node.id);
        })),
    );
    assert!(world.src.has_listeners());
    world.dispatch();
    assert!(world.src.is_empty());
}

#[test]
fn listeners_survive_their_source() {
    let world = World::new();
    let n1 = world.add(1, None);
    let n2 = world.add(2, None);
    let n3 = world.add(3, None);
    {
        let dead: EventSource<dyn Listener> = Default::default();
        n1.listener.attach(&dead);
        n2.listener.attach(&dead);
        n3.listener.attach(&dead);
        assert!(dead.has_listeners());
    }
    n1.listener.attach(&world.src);
    n2.listener.attach(&world.src);
    world.dispatch();
    assert_eq!(world.log(), ["n2", "n1"]);
    drop(n3);
}

#[test]
fn heads_are_recycled() {
    fn round() {
        let world = World::new();
        for id in 0..5 {
            world.add(id, None);
        }
        world.dispatch();
        world.add(
            5,
            Some(Box::new(|node, world| {
                world.nodes.borrow_mut().remove(&node.id);
            })),
        );
        world.dispatch();
    }
    round();
    let before = cached();
    assert!(before > 0);
    for _ in 0..10 {
        round();
        assert_eq!(cached(), before, "a head was lost or leaked");
    }
}

#[test]
fn cache_satisfies_allocations() {
    let listener = Rc::new(Simple);
    let source: EventSource<dyn Listener> = Default::default();
    let listeners: Vec<_> = (0..100)
        .map(|_| EventListener::attached(Rc::downgrade(&listener) as Weak<dyn Listener>, &source))
        .collect();
    drop(listeners);
    drop(source);
    let before = cached();
    assert!(before >= 101);
    let source: EventSource<dyn Listener> = Default::default();
    let listeners: Vec<_> = (0..100)
        .map(|_| EventListener::attached(Rc::downgrade(&listener) as Weak<dyn Listener>, &source))
        .collect();
    assert_eq!(cached(), before - 101);
    source.for_each(|listener| listener.notify());
    drop(listeners);
}

#[test]
fn cache_is_capped_unsized() {
    let listener = Rc::new(Simple);
    let source: EventSource<dyn Listener> = Default::default();
    let listeners: Vec<_> = (0..MAX_CACHED + 100)
        .map(|_| EventListener::attached(Rc::downgrade(&listener) as Weak<dyn Listener>, &source))
        .collect();
    source.for_each(|listener| listener.notify());
    drop(listeners);
    drop(source);
    assert_eq!(cached(), MAX_CACHED);
}

#[test]
fn cache_is_capped_sized() {
    let listener = Rc::new(Simple);
    let source: EventSource<Simple> = Default::default();
    let listeners: Vec<_> = (0..MAX_CACHED + 100)
        .map(|_| EventListener::attached(Rc::downgrade(&listener), &source))
        .collect();
    source.for_each(|listener| listener.notify());
    drop(listeners);
    assert!(source.is_empty());
    drop(source);
    assert_eq!(cached(), MAX_CACHED);
}

#[test]
fn detached_listener_is_not_dispatched_by_nested_pass() {
    let world = World::new();
    let n1 = world.add(1, None);
    let _n2 = world.add(
        2,
        Some(Box::new(move |_node, world| {
            n1.listener.detach();
            let inner = world.clone();
            world.src.for_each(|listener| {
                inner.log.borrow_mut().push("inner".to_string());
                listener.notify();
            });
        })),
    );
    world.dispatch();
    assert_eq!(world.log(), ["n2", "inner", "n2"]);
}
