use crate::utils::aliasable_box::AliasableBox;
use crate::utils::aliasable_box::AliasableBoxExt;
use std::cell::Cell;
use std::rc::Rc;

/// Counts how often it has been dropped.
struct Payload {
    value: u64,
    drops: Rc<Cell<usize>>,
}

impl Drop for Payload {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

fn payload(value: u64) -> (AliasableBox<Payload>, Rc<Cell<usize>>) {
    let drops = Rc::new(Cell::new(0));
    let payload = Payload {
        value,
        drops: drops.clone(),
    };
    (Box::new(payload).into_aliasable(), drops)
}

#[test]
fn deref() {
    let (mut b, _drops) = payload(1);
    assert_eq!(b.value, 1);
    b.value = 2;
    assert_eq!(b.value, 2);
}

#[test]
fn drop_runs_once() {
    let (b, drops) = payload(1);
    assert_eq!(drops.get(), 0);
    drop(b);
    assert_eq!(drops.get(), 1);
}

/// Converting back to a `Box` keeps the allocation and does not drop it twice.
#[test]
fn into_box() {
    let (b, drops) = payload(1);
    let addr = &*b as *const Payload;
    let b = b.into_box();
    assert_eq!(&*b as *const Payload, addr);
    assert_eq!(b.value, 1);
    assert_eq!(drops.get(), 0);
    drop(b);
    assert_eq!(drops.get(), 1);
}

/// The payload does not move when the box does.
///
/// This is the property the type exists for. A raw pointer into the payload
/// stays valid while the box is moved into a struct, through a by-value call,
/// and in and out of a collection.
///
/// The address is all that can be checked from a test. The other half of the
/// guarantee, that none of these moves invalidates the pointer under stacked
/// borrows, is not observable at run time. A `Box` field would be retagged as
/// unique when the struct holding it is constructed by value, which is what
/// `AliasableBox` avoids.
#[test]
fn stable_address() {
    struct Holder {
        ptr: *const u64,
        b: AliasableBox<Payload>,
    }

    #[inline(never)]
    fn take(h: Box<Holder>) -> Box<Holder> {
        h
    }

    let (b, drops) = payload(7);
    let ptr: *const u64 = &b.value;

    let h = take(Box::new(Holder { ptr, b }));
    let mut v = vec![h];
    let h = v.pop().unwrap();

    assert_eq!(unsafe { *h.ptr }, 7);
    assert_eq!(h.ptr, &h.b.value as *const u64);
    assert_eq!(drops.get(), 0);
    drop(h);
    assert_eq!(drops.get(), 1);
}

/// Unsized payloads keep their vtable in both directions.
#[test]
fn unsized_payload() {
    trait Value {
        fn value(&self) -> u64;
    }

    impl Value for Payload {
        fn value(&self) -> u64 {
            self.value
        }
    }

    let drops = Rc::new(Cell::new(0));
    let payload = Payload {
        value: 9,
        drops: drops.clone(),
    };
    let b: Box<dyn Value> = Box::new(payload);
    let b = b.into_aliasable();
    assert_eq!(b.value(), 9);
    let b = b.into_box();
    assert_eq!(b.value(), 9);
    drop(b);
    assert_eq!(drops.get(), 1);
}

#[test]
fn clone() {
    let a = Box::new(5u64).into_aliasable();
    let b = a.clone();
    assert_eq!(*b, 5);
    assert_ne!(&*a as *const u64, &*b as *const u64);
}

/// `clone_from` writes through the existing allocation instead of replacing it,
/// so the address stays stable.
#[test]
fn clone_from() {
    let mut a = Box::new(1u64).into_aliasable();
    let b = Box::new(2u64).into_aliasable();
    let addr = &*a as *const u64;
    a.clone_from(&b);
    assert_eq!(*a, 2);
    assert_eq!(&*a as *const u64, addr);
}

#[test]
fn default_and_debug() {
    let a = AliasableBox::<u64>::default();
    assert_eq!(*a, 0);
    assert_eq!(format!("{a:?}"), "0");
}
