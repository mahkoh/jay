use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;
use std::rc::Rc;
use std::rc::Weak;

#[expect(unused)]
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

#[expect(unused)]
pub trait TypeViewExt1 {
    fn create_view_rc<V>(self: &Rc<Self>) -> &Rc<TypeView<Self, V>>
    where
        V: ?Sized;

    fn create_view_rc_clone<V>(self: &Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized;

    fn into_view_rc<V>(self: Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized;
}

impl<T> TypeViewExt1 for T {
    fn create_view_rc<V>(self: &Rc<Self>) -> &Rc<TypeView<Self, V>>
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

    fn create_view_rc_clone<V>(self: &Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized,
    {
        self.create_view_rc::<V>().clone()
    }

    fn into_view_rc<V>(self: Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized,
    {
        assert_same_layout!(*const Self, Rc<Self>);
        assert_same_layout!(*const Self, Rc<TypeView<Self, V>>);
        // SAFETY: As above.
        unsafe { mem::transmute(self) }
    }
}

#[expect(unused)]
pub trait TypeViewExt2<T>
where
    T: ?Sized,
{
    fn unwrap_view(self: Rc<Self>) -> Rc<T>;
}

impl<T, V> TypeViewExt2<T> for TypeView<T, V>
where
    V: ?Sized,
{
    fn unwrap_view(self: Rc<Self>) -> Rc<T> {
        assert_same_layout!(*const T, Rc<Self>);
        assert_same_layout!(*const T, Rc<T>);
        // SAFETY: As above.
        unsafe { mem::transmute(self) }
    }
}

#[expect(unused)]
pub const fn create_weak_view<T, V>(t: Weak<T>) -> Weak<TypeView<T, V>>
where
    V: ?Sized,
{
    assert_same_layout!(*const T, Weak<T>);
    assert_same_layout!(*const T, Weak<TypeView<T, V>>);
    // SAFETY: As above.
    unsafe { mem::transmute(t) }
}

#[expect(unused)]
pub const fn unwrap_view_rc<T, V>(t: &Rc<TypeView<T, V>>) -> &Rc<T>
where
    V: ?Sized,
{
    assert_same_layout!(*const T, Rc<T>);
    assert_same_layout!(*const T, Rc<TypeView<T, V>>);
    // SAFETY: As above.
    unsafe { mem::transmute(t) }
}
