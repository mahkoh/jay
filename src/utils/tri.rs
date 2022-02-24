use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait Try: Sized {
    fn tri<F>(f: F) -> Result<(), Self>
    where
        F: FnOnce() -> Result<(), Self>;

    fn tria<F>(f: F) -> Tria<Self, F>
    where
        F: Future<Output = Result<(), Self>>;
}

impl<E> Try for E {
    fn tri<F>(f: F) -> Result<(), Self>
    where
        F: FnOnce() -> Result<(), Self>,
    {
        f()
    }

    fn tria<F>(f: F) -> Tria<E, F>
    where
        F: Future<Output = Result<(), Self>>,
    {
        Tria {
            f,
            _phantom: Default::default(),
        }
    }
}

pub struct Tria<E, F> {
    f: F,
    _phantom: PhantomData<E>,
}

impl<E, F> Future for Tria<E, F>
where
    F: Future<Output = Result<(), E>>,
{
    type Output = Result<(), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut Pin::get_unchecked_mut(self).f).poll(cx) }
    }
}
