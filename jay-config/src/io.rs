//! Tools for IO operations.

use {
    crate::_private::PollableId,
    futures_util::{AsyncWrite, io::AsyncRead},
    std::{
        future::poll_fn,
        io::{self, ErrorKind, IoSlice, IoSliceMut, Read, Write},
        os::fd::{AsFd, AsRawFd},
        pin::Pin,
        task::{Context, Poll, ready},
    },
    thiserror::Error,
    uapi::c,
};

#[derive(Debug, Error)]
enum AsyncError {
    #[error("Could not retrieve the file description flags")]
    GetFl(#[source] io::Error),
    #[error("Could not set the file description flags")]
    SetFl(#[source] io::Error),
    #[error("This configuration has already been destroyed")]
    Destroyed,
    #[error("The compositor could not create the necessary data structures: {0}")]
    CompositorSetup(String),
    #[error("Could not poll the file description: {0}")]
    Poll(String),
}

impl From<AsyncError> for io::Error {
    fn from(value: AsyncError) -> Self {
        io::Error::new(ErrorKind::Other, value)
    }
}

/// An async adapter for types implementing [`AsFd`].
pub struct Async<T> {
    id: PollableIdWrapper,
    t: Option<T>,
}

impl<T> Unpin for Async<T> {}

struct PollableIdWrapper {
    id: PollableId,
}

impl Drop for PollableIdWrapper {
    fn drop(&mut self) {
        get!().remove_pollable(self.id);
    }
}

impl<T> Async<T>
where
    T: AsFd,
{
    /// Creates a new async adapter.
    ///
    /// This takes ownership of the file description and duplicates the file descriptor.
    /// You should not modify the file description while this object is in use, otherwise
    /// the behavior is undefined.
    pub fn new(t: T) -> Result<Self, io::Error> {
        Ok(Self::new_(t)?)
    }

    fn new_(t: T) -> Result<Self, AsyncError> {
        let fd = t.as_fd();
        let fl = uapi::fcntl_getfl(fd.as_raw_fd())
            .map_err(|e| AsyncError::GetFl(io::Error::from_raw_os_error(e.0)))?;
        uapi::fcntl_setfl(fd.as_raw_fd(), fl | c::O_NONBLOCK)
            .map_err(|e| AsyncError::SetFl(io::Error::from_raw_os_error(e.0)))?;
        let id = get!(Err(AsyncError::Destroyed))
            .create_pollable(fd.as_raw_fd())
            .map_err(AsyncError::CompositorSetup)?;
        Ok(Self {
            id: PollableIdWrapper { id },
            t: Some(t),
        })
    }
}

impl<T> Async<T> {
    /// Unwraps the underlying object.
    ///
    /// Note that the underlying object is still non-blocking at this point.
    pub fn unwrap(self) -> T {
        self.t.unwrap()
    }

    fn poll_(&self, writable: bool, cx: &mut Context<'_>) -> Poll<Result<(), AsyncError>> {
        get!(Poll::Ready(Err(AsyncError::Destroyed)))
            .poll_io(self.id.id, writable, cx)
            .map_err(AsyncError::Poll)
    }

    async fn poll(&self, writable: bool) -> Result<(), io::Error> {
        poll_fn(|cx| self.poll_(writable, cx)).await?;
        Ok(())
    }

    /// Waits for the file description to become readable.
    pub async fn readable(&self) -> Result<(), io::Error> {
        self.poll(false).await
    }

    /// Waits for the file description to become writable.
    pub async fn writable(&self) -> Result<(), io::Error> {
        self.poll(true).await
    }
}

impl<T> AsRef<T> for Async<T> {
    fn as_ref(&self) -> &T {
        self.t.as_ref().unwrap()
    }
}

impl<T> AsMut<T> for Async<T> {
    fn as_mut(&mut self) -> &mut T {
        self.t.as_mut().unwrap()
    }
}

fn poll_io<T, R>(
    slf: &mut Async<T>,
    writable: bool,
    cx: &mut Context<'_>,
    mut f: impl FnMut(&mut Async<T>) -> io::Result<R>,
) -> Poll<io::Result<R>> {
    loop {
        ready!(slf.poll_(writable, cx))?;
        match f(slf) {
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            res => return Poll::Ready(res),
        }
    }
}

impl<T> AsyncRead for Async<T>
where
    T: Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        poll_io(self.get_mut(), false, cx, |slf| slf.as_mut().read(buf))
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        poll_io(self.get_mut(), false, cx, |slf| {
            slf.as_mut().read_vectored(bufs)
        })
    }
}

impl<T> AsyncWrite for Async<T>
where
    T: Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        poll_io(self.get_mut(), true, cx, |slf| slf.as_mut().write(buf))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        poll_io(self.get_mut(), true, cx, |slf| {
            slf.as_mut().write_vectored(bufs)
        })
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        poll_io(self.get_mut(), true, cx, |slf| slf.as_mut().flush())
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_mut().t.take();
        Poll::Ready(Ok(()))
    }
}
