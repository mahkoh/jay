use crate::utils::type_view::TypeView;
use crate::utils::type_view::TypeViewExt1;
use crate::utils::type_view::TypeViewExt2;
use crate::utils::type_view::tv_unwrap_rc_ref;
use crate::utils::type_view::tv_wrap_weak;
use std::cell::Cell;
use std::fmt::Debug;
use std::rc::Rc;
use std::rc::Weak;

#[derive(Clone, Debug, PartialEq)]
struct Unit;

#[derive(Clone, Debug, PartialEq)]
struct Overaligned(u128, u8);

enum MarkerA {}
enum MarkerB {}

// Exercises every conversion for one concrete (T, V) pair. Instantiating this
// for various T and V is what forces the assert_same_layout! consts in each
// function to be evaluated.
fn smoke<T, V>(t: T)
where
    T: Clone + Debug + PartialEq,
    V: ?Sized,
{
    let rc = Rc::new(t.clone());
    let ptr = Rc::as_ptr(&rc);
    let weak = Rc::downgrade(&rc);

    let view_ref: &Rc<TypeView<T, V>> = rc.tv_wrap_rc_ref::<V>();
    assert_eq!(Rc::as_ptr(view_ref).cast::<T>(), ptr);
    assert_eq!(view_ref.tv_unwrap_ref(), &t);
    assert_eq!(Rc::strong_count(&rc), 1);

    let unwrapped_ref: &Rc<T> = tv_unwrap_rc_ref(view_ref);
    assert_eq!(Rc::as_ptr(unwrapped_ref), ptr);
    assert_eq!(Rc::strong_count(&rc), 1);

    let cloned: Rc<TypeView<T, V>> = rc.tv_wrap_rc_ref_clone::<V>();
    assert_eq!(Rc::as_ptr(&cloned).cast::<T>(), ptr);
    assert_eq!(Rc::strong_count(&rc), 2);
    drop(cloned);
    assert_eq!(Rc::strong_count(&rc), 1);

    let weak_view: Weak<TypeView<T, V>> = tv_wrap_weak::<T, V>(weak);
    assert_eq!(weak_view.strong_count(), 1);
    let upgraded = weak_view.upgrade().unwrap();
    assert_eq!(upgraded.tv_unwrap_ref(), &t);
    assert_eq!(Rc::strong_count(&rc), 2);
    drop(upgraded);

    let view: Rc<TypeView<T, V>> = rc.tv_wrap_rc::<V>();
    assert_eq!(Rc::as_ptr(&view).cast::<T>(), ptr);
    assert_eq!(&**view, &t);

    let back: Rc<T> = view.tv_unwrap_rc();
    assert_eq!(Rc::as_ptr(&back), ptr);
    assert_eq!(*back, t);
    assert_eq!(Rc::strong_count(&back), 1);

    assert_eq!(weak_view.strong_count(), 1);
    drop(back);
    assert_eq!(weak_view.strong_count(), 0);
    assert!(weak_view.upgrade().is_none());
}

#[test]
fn smoke_test() {
    smoke::<u8, MarkerA>(1);
    smoke::<u32, MarkerA>(5);
    smoke::<u32, MarkerB>(5);
    smoke::<u128, MarkerA>(1 << 100);
    smoke::<Unit, MarkerA>(Unit);
    smoke::<Overaligned, MarkerA>(Overaligned(7, 8));
    smoke::<[u8; 33], MarkerA>([9; 33]);
    smoke::<String, MarkerA>("hello".to_string());
    smoke::<Vec<u32>, MarkerA>(vec![1, 2, 3]);
    smoke::<Option<Rc<u32>>, MarkerA>(Some(Rc::new(1)));
}

#[test]
fn smoke_test_unsized_view() {
    smoke::<u32, dyn Debug>(5);
    smoke::<u32, str>(5);
    smoke::<u32, [u8]>(5);
    smoke::<String, dyn Debug>("hello".to_string());
}

#[test]
fn nested_views() {
    let rc = Rc::new(5u32);
    let ptr = Rc::as_ptr(&rc);
    let inner: Rc<TypeView<u32, MarkerA>> = rc.tv_wrap_rc::<MarkerA>();
    let outer: Rc<TypeView<TypeView<u32, MarkerA>, MarkerB>> = inner.tv_wrap_rc::<MarkerB>();
    assert_eq!(Rc::as_ptr(&outer).cast::<u32>(), ptr);
    assert_eq!(***outer, 5);
    let inner = outer.tv_unwrap_rc();
    let rc = inner.tv_unwrap_rc();
    assert_eq!(Rc::as_ptr(&rc), ptr);
    assert_eq!(*rc, 5);
}

#[test]
fn deref() {
    let view = Rc::new("hello".to_string()).tv_wrap_rc::<MarkerA>();
    assert_eq!(view.len(), 5);
    assert_eq!(view.as_str(), "hello");
}

#[test]
fn deref_mut() {
    let mut view = Rc::new(5u32).tv_wrap_rc::<MarkerA>();
    **Rc::get_mut(&mut view).unwrap() = 6;
    assert_eq!(*view.tv_unwrap_ref(), 6);
    assert_eq!(*view.tv_unwrap_rc(), 6);
}

#[test]
fn drop_runs_once() {
    struct Counter(Rc<Cell<usize>>);
    impl Drop for Counter {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let n = Rc::new(Cell::new(0));
    let view = Rc::new(Counter(n.clone())).tv_wrap_rc::<MarkerA>();
    assert_eq!(n.get(), 0);
    let view = view.tv_unwrap_rc().tv_wrap_rc::<MarkerB>();
    assert_eq!(n.get(), 0);
    drop(view);
    assert_eq!(n.get(), 1);
}
