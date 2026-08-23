use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;
use std::rc::Rc;
use std::rc::Weak;

#[cfg(test)]
mod tests;

#[cfg_attr(not(test), expect(unused))]
#[repr(transparent)]
pub struct TypeView<T, V>(PhantomData<fn() -> V>, T)
where
    T: ?Sized,
    V: ?Sized;

impl<T, V> Deref for TypeView<T, V>
where
    T: ?Sized,
    V: ?Sized,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

impl<T, V> DerefMut for TypeView<T, V>
where
    T: ?Sized,
    V: ?Sized,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.1
    }
}

#[cfg_attr(not(test), expect(unused))]
pub trait TypeViewExt1 {
    fn tv_wrap_rc_ref<V>(self: &Rc<Self>) -> &Rc<TypeView<Self, V>>
    where
        V: ?Sized;

    fn tv_wrap_rc_ref_clone<V>(self: &Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized;

    fn tv_wrap_rc<V>(self: Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized;
}

impl<T> TypeViewExt1 for T {
    fn tv_wrap_rc_ref<V>(self: &Rc<Self>) -> &Rc<TypeView<Self, V>>
    where
        V: ?Sized,
    {
        assert_same_layout!(*const Self, Rc<Self>);
        assert_same_layout!(*const Self, Rc<TypeView<Self, V>>);
        // SAFETY: Safety relies on Rc<Self> and Rc<TypeView<Self, V>> having
        // the exact same layout. It follows from the descriptions of the
        // functions Rc::from_raw and Rc::into_raw that the conversion of the
        // owned values via pointers is legal. It is also reasonable to assume
        // that Rc points to an allocation and the legality of the previous
        // transformation therefore implies that the layout of the allocation
        // is the same for both types. However, Rc<Self> and Rc<TypeView<Self,
        // V>> (the values on the stack containing the pointer) might still
        // have different layouts. Above we assert that both have the layout of
        // a pointer, which should imply, unless the authors of Rc are doing
        // something exceedingly clever, that both type layouts are exactly
        // the layout of the pointer to the allocation.
        unsafe { mem::transmute(self) }
    }

    fn tv_wrap_rc_ref_clone<V>(self: &Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized,
    {
        self.tv_wrap_rc_ref::<V>().clone()
    }

    fn tv_wrap_rc<V>(self: Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized,
    {
        assert_same_layout!(*const Self, Rc<Self>);
        assert_same_layout!(*const Self, Rc<TypeView<Self, V>>);
        // SAFETY: As above.
        unsafe { mem::transmute(self) }
    }
}

#[cfg_attr(not(test), expect(unused))]
pub trait TypeViewExt2<T>
where
    T: ?Sized,
{
    fn tv_unwrap_ref(&self) -> &T;
    fn tv_unwrap_rc(self: Rc<Self>) -> Rc<T>;
}

impl<T, V> TypeViewExt2<T> for TypeView<T, V>
where
    V: ?Sized,
{
    fn tv_unwrap_ref(&self) -> &T {
        &self.1
    }

    fn tv_unwrap_rc(self: Rc<Self>) -> Rc<T> {
        assert_same_layout!(*const T, Rc<Self>);
        assert_same_layout!(*const T, Rc<T>);
        // SAFETY: As above.
        unsafe { mem::transmute(self) }
    }
}

#[cfg_attr(not(test), expect(unused))]
pub const fn tv_wrap_weak<T, V>(t: Weak<T>) -> Weak<TypeView<T, V>>
where
    V: ?Sized,
{
    assert_same_layout!(*const T, Weak<T>);
    assert_same_layout!(*const T, Weak<TypeView<T, V>>);
    // SAFETY: As above.
    unsafe { mem::transmute(t) }
}

#[cfg_attr(not(test), expect(unused))]
pub const fn tv_unwrap_rc_ref<T, V>(t: &Rc<TypeView<T, V>>) -> &Rc<T>
where
    V: ?Sized,
{
    assert_same_layout!(*const T, Rc<T>);
    assert_same_layout!(*const T, Rc<TypeView<T, V>>);
    // SAFETY: As above.
    unsafe { mem::transmute(t) }
}
