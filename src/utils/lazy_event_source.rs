use crate::state::State;
use crate::utils::event_listener::EventSource;
use crate::utils::queue::AsyncQueue;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;

pub async fn handle_lazy_event_sources(state: Rc<State>) {
    loop {
        let source = state.lazy_event_sources.queue.pop().await;
        source.queued.set(false);
        for listener in source.listeners.iter() {
            listener.triggered();
        }
    }
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

impl Deref for LazyEventSource {
    type Target = EventSource<dyn LazyEventSourceListener>;

    fn deref(&self) -> &Self::Target {
        &self.listeners
    }
}

impl LazyEventSource {
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
