use {
    crate::{
        state::State,
        utils::{
            linkedlist::{LinkedList, LinkedListIter, LinkedNode},
            queue::AsyncQueue,
        },
    },
    std::{
        cell::Cell,
        ops::Deref,
        rc::{Rc, Weak},
    },
};

pub async fn handle_lazy_event_sources(state: Rc<State>) {
    loop {
        let source = state.lazy_event_sources.queue.pop().await;
        source.queued.set(false);
        for listener in source.listeners.iter() {
            listener.triggered();
        }
    }
}

pub struct EventSource<T: ?Sized> {
    listeners: LinkedList<Weak<T>>,
    on_attach: Cell<Option<Box<dyn FnOnce()>>>,
}

pub struct EventListener<T: ?Sized> {
    link: LinkedNode<Weak<T>>,
}

#[derive(Default)]
pub struct LazyEventSources {
    queue: AsyncQueue<Rc<LazyEventSource>>,
}

pub trait LazyEventSourceListener {
    fn triggered(self: Rc<Self>);
}

pub struct LazyEventSource {
    sources: Rc<LazyEventSources>,
    queued: Cell<bool>,
    listeners: EventSource<dyn LazyEventSourceListener>,
}

impl<T: ?Sized> Default for EventSource<T> {
    fn default() -> Self {
        Self {
            listeners: Default::default(),
            on_attach: Default::default(),
        }
    }
}

impl<T: ?Sized> EventSource<T> {
    pub fn clear(&self) {
        self.on_attach.take();
    }

    pub fn iter(&self) -> EventSourceIter<T> {
        EventSourceIter {
            iter: self.listeners.iter(),
        }
    }

    pub fn has_listeners(&self) -> bool {
        self.listeners.is_not_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }

    pub fn on_attach(&self, f: Box<dyn FnOnce()>) {
        self.on_attach.set(Some(f));
    }
}

pub struct EventSourceIter<T: ?Sized> {
    iter: LinkedListIter<Weak<T>>,
}

impl<T: ?Sized> Iterator for EventSourceIter<T> {
    type Item = Rc<T>;

    fn next(&mut self) -> Option<Self::Item> {
        for weak in self.iter.by_ref() {
            if let Some(t) = weak.upgrade() {
                return Some(t);
            }
        }
        None
    }
}

impl<T: ?Sized> EventListener<T> {
    pub fn new(t: Weak<T>) -> Self {
        Self {
            link: LinkedNode::detached(t),
        }
    }

    pub fn attach(&self, source: &EventSource<T>) {
        source.listeners.add_last_existing(&self.link);
        if let Some(on_attach) = source.on_attach.take() {
            on_attach();
        }
    }

    pub fn detach(&self) {
        self.link.detach();
    }

    pub fn get(&self) -> Option<Rc<T>> {
        self.link.upgrade()
    }
}

impl Deref for LazyEventSource {
    type Target = EventSource<dyn LazyEventSourceListener>;

    fn deref(&self) -> &Self::Target {
        &self.listeners
    }
}

impl LazyEventSource {
    #[expect(dead_code)]
    pub fn trigger(self: &Rc<Self>) {
        if self.listeners.is_empty() {
            return;
        }
        if self.queued.replace(true) {
            return;
        }
        self.sources.queue.push(self.clone());
    }
}

impl LazyEventSources {
    #[expect(dead_code)]
    pub fn create_source(self: &Rc<Self>) -> Rc<LazyEventSource> {
        Rc::new(LazyEventSource {
            sources: self.clone(),
            queued: Default::default(),
            listeners: Default::default(),
        })
    }

    pub fn clear(&self) {
        self.queue.clear();
    }
}
