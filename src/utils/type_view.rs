use std::marker::PhantomData;
use std::ops::Deref;
use std::ops::DerefMut;
use std::rc::Rc;
use std::rc::Weak;

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
    fn create_view_rc<V>(self: &Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized;

    fn create_view_weak<V>(self: &Rc<Self>) -> Weak<TypeView<Self, V>>
    where
        V: ?Sized;
}

impl<T> TypeViewExt1 for T {
    fn create_view_rc<V>(self: &Rc<Self>) -> Rc<TypeView<Self, V>>
    where
        V: ?Sized,
    {
        unsafe { Rc::from_raw(Rc::into_raw(self.clone()).cast()) }
    }

    fn create_view_weak<V>(self: &Rc<Self>) -> Weak<TypeView<Self, V>>
    where
        V: ?Sized,
    {
        create_weak_view(Rc::downgrade(self))
    }
}

pub fn create_weak_view<T, V>(t: Weak<T>) -> Weak<TypeView<T, V>>
where
    V: ?Sized,
{
    unsafe { Weak::from_raw(Weak::into_raw(t).cast()) }
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
        unsafe { Rc::from_raw(Rc::into_raw(self).cast()) }
    }
}
