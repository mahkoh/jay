use crate::async_engine::AsyncEngine;
use crate::async_engine::SpawnedFuture;
use crate::io_uring::IoUring;
use crate::io_uring::IoUringError;
use crate::io_uring::ops::writev_external::WritevData;
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::oserror::OsError;
use crate::utils::pipe::Pipe;
use crate::utils::pipe::pipe;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;
use uapi::c::iovec;

fn new_pipe() -> Pipe<Rc<OwnedFd>, Rc<OwnedFd>> {
    pipe().unwrap().map_read(Rc::new).map_write(Rc::new)
}

/// The outcome of a write, observable after the writer has been consumed.
#[derive(Default)]
struct Shared {
    ae: AsyncEvent,
    res: RefCell<Option<Result<usize, OsError>>>,
    /// The number of times the writer has been dropped.
    drops: Cell<usize>,
}

impl Shared {
    async fn wait(&self) {
        while self.res.borrow().is_none() {
            self.ae.triggered().await;
        }
    }

    fn take(&self) -> Result<usize, OsError> {
        self.res
            .borrow_mut()
            .take()
            .expect("write did not complete")
    }
}

/// Writes the buffers it owns.
struct Writer {
    shared: Rc<Shared>,
    /// Points into `bufs`.
    iovecs: Vec<iovec>,
    bufs: Vec<Vec<u8>>,
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.shared.drops.set(self.shared.drops.get() + 1);
    }
}

unsafe impl WritevData for Writer {
    fn iovecs(&self) -> &[iovec] {
        &self.iovecs
    }

    fn done(self: Box<Self>, res: Result<usize, OsError>) {
        *self.shared.res.borrow_mut() = Some(res);
        self.shared.ae.trigger();
    }
}

impl Writer {
    fn new(chunks: &[&[u8]]) -> (Box<Self>, Rc<Shared>) {
        let shared = Rc::new(Shared::default());
        let mut writer = Box::new(Writer {
            shared: shared.clone(),
            iovecs: vec![],
            bufs: chunks.iter().map(|c| c.to_vec()).collect(),
        });
        // The iovecs are derived only after the buffers are owned by the box so
        // that the addresses they store no longer change.
        let iovecs = writer
            .bufs
            .iter()
            .map(|b| iovec {
                iov_base: b.as_ptr().cast_mut().cast(),
                iov_len: b.len(),
            })
            .collect();
        writer.iovecs = iovecs;
        (writer, shared)
    }
}

fn watchdog(eng: &Rc<AsyncEngine>, ring: &Rc<IoUring>) -> SpawnedFuture<()> {
    let deadline = eng.now().nsec() + 10_000_000_000;
    let ring = ring.clone();
    eng.spawn("watchdog", async move {
        let _ = ring.timeout(deadline).await;
        ring.stop();
    })
}

fn read_all(fd: &Rc<OwnedFd>) -> Vec<u8> {
    let mut buf = [0u8; 128];
    uapi::read(fd.raw(), &mut buf[..]).unwrap().to_vec()
}

/// The iovecs are written in order and the writer is consumed.
#[test]
fn write() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let Pipe { read, write } = new_pipe();
    // Including an empty iovec in the middle.
    let chunks: &[&[u8]] = &[b"aaaa", b"", b"bb", b"cccccc"];
    let (writer, shared) = Writer::new(chunks);
    ring.writev_external(&write, writer).unwrap();

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let shared2 = shared.clone();
    let _fut = eng.spawn("", async move {
        shared2.wait().await;
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(shared.take(), Ok(12));
    assert_eq!(read_all(&read), b"aaaabbcccccc");
    // The writer was dropped when the write completed.
    assert_eq!(shared.drops.get(), 1);
}

/// A write without iovecs completes without writing anything.
#[test]
fn no_iovecs() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let Pipe { read: _read, write } = new_pipe();
    let (writer, shared) = Writer::new(&[]);
    ring.writev_external(&write, writer).unwrap();

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let shared2 = shared.clone();
    let _fut = eng.spawn("", async move {
        shared2.wait().await;
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(shared.take(), Ok(0));
    assert_eq!(shared.drops.get(), 1);
}

/// A second write reuses the cached task without inheriting its state.
#[test]
fn task_reuse() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let Pipe { read, write } = new_pipe();

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let write2 = write.clone();
    let outcomes = Rc::new(RefCell::new(vec![]));
    let outcomes2 = outcomes.clone();
    let _fut = eng.spawn("", async move {
        // Different fds, iovec counts and lengths, so that a field left over
        // from the first write would show up in the second one.
        let chunks: &[&[u8]] = &[b"aaaa", b"bbbb", b"cccc"];
        let (writer, shared) = Writer::new(chunks);
        ring2.writev_external(&write2, writer).unwrap();
        shared.wait().await;
        outcomes2.borrow_mut().push(shared.take());

        let Pipe {
            read: read2,
            write: write3,
        } = new_pipe();
        let chunks: &[&[u8]] = &[b"d"];
        let (writer, shared) = Writer::new(chunks);
        ring2.writev_external(&write3, writer).unwrap();
        shared.wait().await;
        outcomes2.borrow_mut().push(shared.take());
        assert_eq!(read_all(&read2), b"d");

        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(*outcomes.borrow(), vec![Ok(12), Ok(1)]);
    assert_eq!(read_all(&read), b"aaaabbbbcccc");
}

/// Errors are reported to the writer.
#[test]
fn error() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    // Not open for writing.
    let fd = Rc::new(uapi::open("/dev/null", c::O_RDONLY | c::O_CLOEXEC, 0).unwrap());
    let (writer, shared) = Writer::new(&[&b"aaaa"[..]]);
    ring.writev_external(&fd, writer).unwrap();

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let shared2 = shared.clone();
    let _fut = eng.spawn("", async move {
        shared2.wait().await;
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(shared.take(), Err(OsError(c::EBADF)));
    assert_eq!(shared.drops.get(), 1);
}

/// The write keeps the file alive until it completes.
#[test]
fn fd_kept_alive() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let Pipe { read, write } = new_pipe();
    let (writer, shared) = Writer::new(&[&b"aaaa"[..]]);
    ring.writev_external(&write, writer).unwrap();
    // The caller no longer holds the file. If the write did not keep it alive,
    // it would fail with EBADF.
    drop(write);

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let shared2 = shared.clone();
    let _fut = eng.spawn("", async move {
        shared2.wait().await;
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(shared.take(), Ok(4));
    assert_eq!(read_all(&read), b"aaaa");
}

/// A write that has not been submitted to the kernel is cancelled when the ring
/// is destroyed.
#[test]
fn cancelled() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let Pipe { read: _read, write } = new_pipe();
    let (writer, shared) = Writer::new(&[&b"aaaa"[..]]);
    ring.writev_external(&write, writer).unwrap();
    // The ring was never run, so the sqe has not been encoded yet.
    ring.stop();

    assert_eq!(shared.take(), Err(OsError(c::ECANCELED)));
    assert_eq!(shared.drops.get(), 1);
}

/// A write cannot be scheduled on a destroyed ring.
#[test]
fn destroyed() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let Pipe { read: _read, write } = new_pipe();
    ring.stop();

    let (writer, shared) = Writer::new(&[&b"aaaa"[..]]);
    let res = ring.writev_external(&write, writer);
    assert!(matches!(res, Err(IoUringError::Destroyed)));
    // The writer is not consumed, so it is dropped without being completed.
    assert_eq!(shared.drops.get(), 1);
    assert!(shared.res.borrow().is_none());
}
