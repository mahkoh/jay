use crate::utils::keep_alive::KeepAlive;
use crate::utils::ptr_ext::PtrExt;
use static_assertions::const_assert;
use std::cell::Cell;
use std::mem::MaybeUninit;
use std::ptr;
use std::rc::Rc;
use std::rc::Weak;

#[cfg(test)]
mod tests;

pub struct EventSource<T>
where
    T: ?Sized,
{
    listeners: *const Head<T>,
    on_attach: Cell<Option<Box<dyn FnOnce()>>>,
}

impl<T> Default for EventSource<T>
where
    T: ?Sized,
{
    fn default() -> Self {
        Self {
            listeners: allocate_head(None),
            on_attach: Default::default(),
        }
    }
}

pub struct EventListener<T>
where
    T: ?Sized,
{
    head: Cell<*const Head<T>>,
}

const MAX_CACHED: usize = 1 << 13;

thread_local! {
    static CACHE: Cell<Cache> = const {
        Cell::new(Cache {
            num_heads: 0,
            heads: ptr::null_mut(),
        })
    };
}

#[derive(Copy, Clone)]
struct Cache {
    num_heads: usize,
    heads: *mut Cached,
}

struct Cached {
    next: *mut Cached,
}

type CachedHead = Head<dyn KeepAlive>;

struct Head<T>
where
    T: ?Sized,
{
    prev: Cell<*const Head<T>>,
    next: Cell<*const Head<T>>,
    listener: Option<Weak<T>>,
    pinned: Cell<bool>,
    dead: Cell<bool>,
}

unsafe fn release_head<T>(head_ptr: *const Head<T>)
where
    T: ?Sized,
{
    const_assert!(size_of::<Cached>() <= size_of::<CachedHead>());
    const_assert!(align_of::<Cached>() <= align_of::<CachedHead>());
    unsafe {
        {
            let head_ref = head_ptr.deref();
            debug_assert!(!head_ref.pinned.get());
            head_ref.connect_neighbors();
        }
        let head_ptr = head_ptr.cast_mut();
        ptr::drop_in_place(head_ptr);
        let mut cache = CACHE.get();
        if cache.num_heads >= MAX_CACHED {
            release_head_slow(head_ptr);
        } else {
            let cached = head_ptr.cast::<Cached>();
            ptr::write(cached, Cached { next: cache.heads });
            cache.heads = cached;
            cache.num_heads += 1;
            CACHE.set(cache);
        }
    }
}

#[cold]
unsafe fn release_head_slow<T>(head_ptr: *mut Head<T>)
where
    T: ?Sized,
{
    unsafe {
        let _ = Box::from_raw(head_ptr.cast::<MaybeUninit<CachedHead>>());
    }
}

fn allocate_head<T>(listener: Option<Weak<T>>) -> *const Head<T>
where
    T: ?Sized,
{
    const {
        assert!(size_of::<Head<T>>() <= size_of::<CachedHead>());
        assert!(align_of::<Head<T>>() <= align_of::<CachedHead>());
    }
    let mut cache = CACHE.get();
    let cached = cache.heads;
    if cached.is_null() {
        return allocate_head_slow(listener);
    }
    let next = unsafe { cached.deref() }.next;
    cache.heads = next;
    cache.num_heads -= 1;
    CACHE.set(cache);
    let head_ptr = cached.cast::<Head<T>>();
    init_head(head_ptr, listener);
    head_ptr
}

fn init_head<T>(head: *mut Head<T>, listener: Option<Weak<T>>)
where
    T: ?Sized,
{
    unsafe {
        ptr::write(
            head,
            Head {
                pinned: Default::default(),
                dead: Default::default(),
                prev: Cell::new(head),
                next: Cell::new(head),
                listener,
            },
        );
    }
}

#[cold]
fn allocate_head_slow<T>(listener: Option<Weak<T>>) -> *mut Head<T>
where
    T: ?Sized,
{
    let ptr = Box::into_raw(Box::<CachedHead>::new_uninit());
    let head_ptr = ptr.cast::<Head<T>>();
    init_head(head_ptr, listener);
    head_ptr
}

impl<T> EventSource<T>
where
    T: ?Sized,
{
    pub fn clear(&self) {
        self.on_attach.take();
    }

    pub fn for_each(&self, mut f: impl FnMut(Rc<T>)) {
        unsafe {
            let mut next_ptr = self.listeners.deref().next.get();
            let mut next_ref = next_ptr.deref();
            let mut next_pinned = next_ref.pinned.replace(true);
            loop {
                let cur_ptr = next_ptr;
                let cur_ref = next_ref;
                let cur_pinned = next_pinned;
                let Some(cur_weak) = &cur_ref.listener else {
                    break;
                };
                next_ptr = cur_ref.next.get();
                next_ref = next_ptr.deref();
                next_pinned = next_ref.pinned.replace(true);
                cur_ref.pinned.set(cur_pinned);
                if cur_ref.dead.get() {
                    if !cur_pinned {
                        release_head(cur_ptr);
                    }
                    continue;
                }
                if let Some(cur_strong) = cur_weak.upgrade() {
                    f(cur_strong);
                }
            }
        }
    }

    pub fn has_listeners(&self) -> bool {
        !self.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.listeners.deref().next.get() == self.listeners }
    }

    pub fn on_attach(&self, f: Box<dyn FnOnce()>) {
        self.on_attach.set(Some(f));
    }
}

impl<T> Head<T>
where
    T: ?Sized,
{
    #[inline(always)]
    fn connect_neighbors(&self) {
        let prev = self.prev.get();
        let next = self.next.get();
        unsafe {
            prev.deref().next.set(next);
            next.deref().prev.set(prev);
        }
    }

    #[inline(always)]
    fn detach(&self, slf: *const Self) {
        self.connect_neighbors();
        self.next.set(slf);
        self.prev.set(slf);
    }
}

impl<T> EventListener<T>
where
    T: ?Sized,
{
    pub fn new(t: Weak<T>) -> Self {
        Self {
            head: Cell::new(allocate_head(Some(t))),
        }
    }

    pub fn attached(t: Weak<T>, source: &EventSource<T>) -> Self {
        let slf = Self::new(t);
        slf.attach(source);
        slf
    }

    fn unpin(&self) {
        unsafe {
            let head_ref = self.head.get().deref();
            if head_ref.pinned.get() {
                self.unpin_slow(head_ref);
            }
        }
    }

    #[cold]
    fn unpin_slow(&self, head_ref: &Head<T>) {
        head_ref.dead.set(true);
        let head_ptr = allocate_head(head_ref.listener.clone());
        self.head.set(head_ptr);
    }

    pub fn attach(&self, source: &EventSource<T>) {
        self.unpin();
        unsafe {
            let src_ptr = self.head.get();
            let src = src_ptr.deref();
            src.connect_neighbors();
            let dst = source.listeners.deref();
            let nxt = dst.next.get();
            src.prev.set(dst);
            src.next.set(nxt);
            nxt.deref().prev.set(src_ptr);
            dst.next.set(src_ptr);
        }
        if let Some(on_attach) = source.on_attach.take() {
            on_attach();
        }
    }

    pub fn detach(&self) {
        self.unpin();
        let head_ptr = self.head.get();
        unsafe {
            head_ptr.deref().detach(head_ptr);
        }
    }

    pub fn get(&self) -> Option<Rc<T>> {
        let head_ptr = self.head.get();
        unsafe { head_ptr.deref().listener.as_ref().and_then(Weak::upgrade) }
    }
}

impl<T> Drop for EventListener<T>
where
    T: ?Sized,
{
    fn drop(&mut self) {
        unsafe {
            let head_ptr = self.head.get();
            let head_ref = head_ptr.deref();
            if head_ref.pinned.get() {
                head_ref.dead.set(true);
            } else {
                release_head(head_ptr);
            }
        }
    }
}

impl<T> Drop for EventSource<T>
where
    T: ?Sized,
{
    fn drop(&mut self) {
        unsafe {
            self.listeners.deref().pinned.set(false);
            release_head(self.listeners);
        }
    }
}
