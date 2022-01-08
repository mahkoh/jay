use crate::utils::ptr_ext::PtrExt;
use crate::NumCell;
use std::cell::Cell;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ptr::NonNull;

pub struct LinkedList<T> {
    root: Node<T>,
}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        let node = Box::into_raw(Box::new(NodeData {
            rc: NumCell::new(3),
            prev: Cell::new(NonNull::dangling()),
            next: Cell::new(NonNull::dangling()),
            data: MaybeUninit::uninit(),
        }));
        unsafe {
            node.deref().prev.set(NonNull::new_unchecked(node));
            node.deref().next.set(NonNull::new_unchecked(node));
            Self {
                root: Node {
                    data: NonNull::new_unchecked(node),
                },
            }
        }
    }

    pub fn add_last(&self, t: T) -> Node<T> {
        self.root.prepend(t)
    }

    pub fn add_first(&self, t: T) -> Node<T> {
        self.root.append(t)
    }

    pub fn iter(&self) -> LinkedListIter<T> {
        unsafe {
            let root = self.root.data.as_ref();
            root.rc.fetch_add(1);
            root.next.get().as_ref().rc.fetch_add(1);
            LinkedListIter {
                root: self.root.data,
                next: root.next.get(),
            }
        }
    }

    pub fn rev_iter(&self) -> RevLinkedListIter<T> {
        unsafe {
            let root = self.root.data.as_ref();
            root.rc.fetch_add(1);
            root.prev.get().as_ref().rc.fetch_add(1);
            RevLinkedListIter {
                root: self.root.data,
                next: root.prev.get(),
            }
        }
    }
}

pub struct LinkedListIter<T> {
    root: NonNull<NodeData<T>>,
    next: NonNull<NodeData<T>>,
}

impl<T> Iterator for LinkedListIter<T> {
    type Item = NodeRef<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.root == self.next {
            return None;
        }
        unsafe {
            let old_next = self.next;
            self.next = old_next.as_ref().next.get();
            self.next.as_ref().rc.fetch_add(1);
            Some(NodeRef { data: old_next })
        }
    }
}

impl<T> Drop for LinkedListIter<T> {
    fn drop(&mut self) {
        unsafe {
            dec_ref_count(self.root, 1);
            dec_ref_count(self.next, 1);
        }
    }
}

pub struct RevLinkedListIter<T> {
    root: NonNull<NodeData<T>>,
    next: NonNull<NodeData<T>>,
}

impl<T> Iterator for RevLinkedListIter<T> {
    type Item = NodeRef<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.root == self.next {
            return None;
        }
        unsafe {
            let old_next = self.next;
            self.next = old_next.as_ref().prev.get();
            self.next.as_ref().rc.fetch_add(1);
            Some(NodeRef { data: old_next })
        }
    }
}

impl<T> Drop for RevLinkedListIter<T> {
    fn drop(&mut self) {
        unsafe {
            dec_ref_count(self.root, 1);
            dec_ref_count(self.next, 1);
        }
    }
}

pub struct Node<T> {
    data: NonNull<NodeData<T>>,
}

impl<T> Deref for Node<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref().data.assume_init_ref() }
    }
}

pub struct NodeRef<T> {
    data: NonNull<NodeData<T>>,
}

impl<T> Deref for NodeRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref().data.assume_init_ref() }
    }
}

impl<T> Drop for NodeRef<T> {
    fn drop(&mut self) {
        unsafe {
            dec_ref_count(self.data, 1);
        }
    }
}

struct NodeData<T> {
    rc: NumCell<usize>,
    prev: Cell<NonNull<NodeData<T>>>,
    next: Cell<NonNull<NodeData<T>>>,
    data: MaybeUninit<T>,
}

unsafe fn dec_ref_count<T>(slf: NonNull<NodeData<T>>, n: usize) {
    if slf.as_ref().rc.fetch_sub(n) == n {
        drop(Box::from_raw(slf.as_ptr()));
    }
}

impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        unsafe {
            {
                let data = self.data.as_ref();
                data.prev.get().as_ref().next.set(data.next.get());
                data.next.get().as_ref().prev.set(data.prev.get());
            }
            dec_ref_count(self.data, 3);
        }
    }
}

impl<T> Node<T> {
    pub fn prepend(&self, t: T) -> Node<T> {
        unsafe {
            let data = self.data.as_ref();
            let node = NonNull::new_unchecked(Box::into_raw(Box::new(NodeData {
                rc: NumCell::new(3),
                prev: Cell::new(data.prev.get()),
                next: Cell::new(self.data),
                data: MaybeUninit::new(t),
            })));
            data.prev.get().as_ref().next.set(node);
            data.prev.set(node);
            Node { data: node }
        }
    }

    pub fn append(&self, t: T) -> Node<T> {
        unsafe {
            let data = self.data.as_ref();
            let node = NonNull::new_unchecked(Box::into_raw(Box::new(NodeData {
                rc: NumCell::new(3),
                prev: Cell::new(self.data),
                next: Cell::new(data.next.get()),
                data: MaybeUninit::new(t),
            })));
            data.next.get().as_ref().prev.set(node);
            data.next.set(node);
            Node { data: node }
        }
    }
}
