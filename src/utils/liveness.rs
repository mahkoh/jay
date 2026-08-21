use crate::utils::ptr_ext::MutPtrExt;
use std::cell::UnsafeCell;
use std::mem::ManuallyDrop;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;

thread_local! {
    static M: ManuallyDrop<UnsafeCell<Vec<&'static AtomicU64>>> = const {
        ManuallyDrop::new(UnsafeCell::new(Vec::new()))
    };
}

#[expect(unused)]
pub static STATIC_LIVENESS: &Liveness = {
    static S: AtomicU64 = AtomicU64::new(0);
    static L: Liveness = Liveness { s: &S };
    &L
};

#[derive(Debug)]
pub struct Liveness {
    s: &'static AtomicU64,
}

#[derive(Copy, Clone)]
pub struct LivenessView {
    s: &'static AtomicU64,
    v: u64,
}

impl Default for Liveness {
    fn default() -> Self {
        M.with(|q| {
            let q = unsafe { q.get().deref_mut() };
            if let Some(s) = q.pop() {
                return Self { s };
            }
            new_slow()
        })
    }
}

impl Liveness {
    #[expect(unused)]
    pub fn view(&self) -> LivenessView {
        LivenessView {
            s: self.s,
            v: self.s.load(Relaxed),
        }
    }
}

impl LivenessView {
    #[expect(unused)]
    pub fn is_alive(&self) -> bool {
        self.s.load(Relaxed) == self.v
    }

    #[expect(unused)]
    pub fn is_dead(&self) -> bool {
        self.s.load(Relaxed) != self.v
    }
}

#[cold]
fn new_slow() -> Liveness {
    M.with(|q| {
        let q = unsafe { q.get().deref_mut() };
        let v: Vec<_> = (0..512).map(|_| AtomicU64::new(0)).collect();
        let v: &'static [AtomicU64] = Box::leak(v.into_boxed_slice());
        q.extend(v);
        let s = q.pop().unwrap();
        Liveness { s }
    })
}

#[cold]
fn old_slow(s: &'static AtomicU64) {
    M.with(|q| {
        let q = unsafe { q.get().deref_mut() };
        q.push(s);
    });
}

impl Drop for Liveness {
    #[inline]
    fn drop(&mut self) {
        let v = self.s.load(Relaxed);
        self.s.store(v.wrapping_add(1), Relaxed);
        M.with(|q| {
            let q = unsafe { q.get().deref_mut() };
            if q.len() < q.capacity() {
                q.push(self.s);
            } else {
                old_slow(self.s);
            }
        });
    }
}
