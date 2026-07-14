use std::marker::PhantomData;

pub struct AssertJayClone<T: JayClone>(PhantomData<T>);

pub unsafe trait JayClone: Clone {
    fn _assert(&self) {
        // nothing
    }
}

mod impls {
    use {
        crate::{tree::NodeId, utils::markers::JayClone},
        jay_config::{keyboard::mods::Modifiers, window::Window},
        std::{
            rc::{Rc, Weak},
            sync::Arc,
        },
    };

    unsafe impl<T: JayClone> JayClone for Option<T> {}

    unsafe impl<T: ?Sized> JayClone for Rc<T> {}
    unsafe impl<T: ?Sized> JayClone for Weak<T> {}
    unsafe impl<T: ?Sized> JayClone for Arc<T> {}

    unsafe impl JayClone for () {}
    unsafe impl JayClone for u64 {}
    unsafe impl JayClone for i32 {}
    unsafe impl JayClone for u32 {}
    unsafe impl JayClone for usize {}
    unsafe impl JayClone for f32 {}

    unsafe impl<A: JayClone, B: JayClone> JayClone for (A, B) {}
    unsafe impl<T: JayClone, const N: usize> JayClone for [T; N] {}

    unsafe impl JayClone for Modifiers {}

    unsafe impl JayClone for NodeId {}

    unsafe impl JayClone for Window {}
}
