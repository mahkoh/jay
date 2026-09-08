use crate::utils::linkedlist::LinkedList;
use crate::utils::linkedlist::LinkedListIter;
use crate::utils::linkedlist::LinkedNode;
use derivative::Derivative;
use std::cell::Cell;
use std::rc::Rc;
use std::rc::Weak;

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct EventSource<T: ?Sized> {
    listeners: LinkedList<Weak<T>>,
    on_attach: Cell<Option<Box<dyn FnOnce()>>>,
}

pub struct EventListener<T: ?Sized> {
    link: LinkedNode<Weak<T>>,
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
