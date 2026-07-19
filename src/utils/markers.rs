use std::hash::Hash;
use std::marker::PhantomData;

pub struct AssertJayClone<T: JayClone>(PhantomData<T>);

pub unsafe trait JayClone: Clone {
    fn _assert(&self) {
        // nothing
    }
}

#[expect(dead_code)]
pub struct AssertJayHash<T: JayHash>(PhantomData<T>);

pub unsafe trait JayHash: Hash + PartialEq {
    fn _assert(&self) {
        // nothing
    }
}

mod impls {
    use crate::tree::NodeId;
    use crate::utils::markers::JayClone;
    use crate::utils::markers::JayHash;
    use jay_config::_private::ClientCriterionIpc;
    use jay_config::_private::PollableId;
    use jay_config::_private::WindowCriterionIpc;
    use jay_config::client::ClientMatcher;
    use jay_config::keyboard::Keymap;
    use jay_config::keyboard::ModifiedKeySym;
    use jay_config::keyboard::mods::Modifiers;
    use jay_config::window::Window;
    use jay_config::window::WindowMatcher;
    use kbvm::Keycode;
    use std::borrow::Cow;
    use std::rc::Rc;
    use std::rc::Weak;
    use std::sync::Arc;

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

    unsafe impl JayHash for u8 {}
    unsafe impl JayHash for u16 {}
    unsafe impl JayHash for i32 {}
    unsafe impl JayHash for u32 {}
    unsafe impl JayHash for u64 {}
    unsafe impl JayHash for usize {}
    unsafe impl JayHash for str {}
    unsafe impl JayHash for bool {}
    unsafe impl JayHash for String {}
    unsafe impl JayHash for &'_ str {}
    unsafe impl JayHash for Cow<'_, str> {}
    unsafe impl JayHash for ash::vk::Format {}
    unsafe impl<T> JayHash for Option<T> where T: JayHash {}
    unsafe impl<T> JayHash for Rc<T> where T: JayHash {}
    unsafe impl JayHash for ClientCriterionIpc {}
    unsafe impl JayHash for WindowCriterionIpc {}
    unsafe impl JayHash for Keymap {}
    unsafe impl JayHash for PollableId {}
    unsafe impl JayHash for Window {}
    unsafe impl JayHash for ClientMatcher {}
    unsafe impl JayHash for WindowMatcher {}
    unsafe impl JayHash for Keycode {}
    unsafe impl JayHash for ModifiedKeySym {}
    unsafe impl<T, U> JayHash for (T, U)
    where
        T: JayHash,
        U: JayHash,
    {
    }
    unsafe impl<T, U, V> JayHash for (T, U, V)
    where
        T: JayHash,
        U: JayHash,
        V: JayHash,
    {
    }
    unsafe impl<T, const N: usize> JayHash for [T; N] where T: JayHash {}
}
