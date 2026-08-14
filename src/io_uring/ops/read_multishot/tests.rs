use crate::async_engine::AsyncEngine;
use crate::async_engine::SpawnedFuture;
use crate::io_uring::IoUring;
use crate::io_uring::IoUringError;
use crate::io_uring::buffer_ring::BufferRing;
use crate::io_uring::buffer_ring::BufferRingBuffer;
use crate::io_uring::buffer_ring::BufferRingError;
use crate::io_uring::ops::read_multishot::PendingReadMultishot;
use crate::io_uring::ops::read_multishot::ReadMultishotCallback;
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::oserror::OsError;
use crate::utils::pipe::Pipe;
use crate::utils::pipe::pipe;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;

const NUM_BUFFERS: usize = 2;
const BUFFER_SIZE: usize = 4;

fn new_pipe() -> Pipe<Rc<OwnedFd>, Rc<OwnedFd>> {
    pipe().unwrap().map_read(Rc::new).map_write(Rc::new)
}

/// The reason a read stopped.
#[derive(Debug, Eq, PartialEq)]
enum Outcome {
    Eof,
    NoBuffers,
    Error(OsError),
}

#[derive(Debug, Eq, PartialEq)]
struct Chunk {
    data: Vec<u8>,
    /// The address of the buffer the chunk was delivered in.
    addr: usize,
    /// The ring iteration in which the chunk was delivered.
    iteration: u64,
    /// The number of sqes that had been submitted when it was delivered.
    arm: u64,
}

/// Reads a file until it stops producing data.
///
/// The kernel can deliver data either in an intermediate completion or in the
/// final completion of a read. In the latter case the read is no longer armed
/// and has to be armed again to receive the rest of the data. Both cases funnel
/// into `chunks`.
struct Reader {
    ring: Rc<IoUring>,
    fd: Rc<OwnedFd>,
    buffer_ring: Rc<BufferRing>,
    pending: Cell<Option<PendingReadMultishot>>,
    arms: Cell<u64>,

    chunk_ae: AsyncEvent,
    chunks: RefCell<Vec<Chunk>>,

    outcome_ae: AsyncEvent,
    outcome: RefCell<Option<Outcome>>,

    /// Retain the buffers instead of returning them to the ring on drop.
    keep_buffers: Cell<bool>,
    kept: RefCell<Vec<BufferRingBuffer>>,
    /// Cancel the read from within the callback of the first chunk.
    cancel_on_first_chunk: Cell<bool>,
}

impl ReadMultishotCallback for Reader {
    fn read(self: Rc<Self>, res: BufferRingBuffer) {
        self.push(res);
    }

    fn done(self: Rc<Self>, res: Result<Option<BufferRingBuffer>, OsError>) {
        match res {
            Ok(Some(buf)) if !buf.is_empty() => {
                self.push(buf);
                // Unless the read was just cancelled, arm it again to receive
                // the rest of the data.
                if self.pending.take().is_some() {
                    self.arm();
                }
            }
            Ok(Some(_)) => self.set_outcome(Outcome::Eof),
            Ok(None) => self.set_outcome(Outcome::NoBuffers),
            Err(e) => self.set_outcome(Outcome::Error(e)),
        }
    }
}

impl Reader {
    fn new(ring: &Rc<IoUring>, fd: &Rc<OwnedFd>, buffer_ring: &Rc<BufferRing>) -> Rc<Self> {
        Rc::new(Self {
            ring: ring.clone(),
            fd: fd.clone(),
            buffer_ring: buffer_ring.clone(),
            pending: Default::default(),
            arms: Default::default(),
            chunk_ae: Default::default(),
            chunks: Default::default(),
            outcome_ae: Default::default(),
            outcome: Default::default(),
            keep_buffers: Default::default(),
            kept: Default::default(),
            cancel_on_first_chunk: Default::default(),
        })
    }

    /// Returns whether the read was armed. It is not armed if the ring is
    /// already shutting down.
    fn arm(self: &Rc<Self>) -> bool {
        let pending = match self
            .ring
            .read_multishot(&self.fd, &self.buffer_ring, self.clone())
        {
            Ok(p) => p,
            Err(IoUringError::Destroyed) => return false,
            Err(e) => panic!("could not arm read: {}", ErrorFmt(e)),
        };
        self.arms.set(self.arms.get() + 1);
        self.pending.set(Some(pending));
        true
    }

    fn cancel(&self) {
        self.pending.take();
    }

    /// Returns the retained buffers to the buffer ring and clears the outcome.
    fn release_buffers(&self) -> Option<Outcome> {
        self.kept.borrow_mut().clear();
        self.outcome.borrow_mut().take()
    }

    fn push(self: &Rc<Self>, buf: BufferRingBuffer) {
        self.chunks.borrow_mut().push(Chunk {
            data: buf.to_vec(),
            addr: buf.as_ptr() as usize,
            iteration: self.ring.ring.iteration.get(),
            arm: self.arms.get(),
        });
        if self.keep_buffers.get() {
            self.kept.borrow_mut().push(buf);
        }
        if self.cancel_on_first_chunk.take() {
            self.cancel();
        }
        self.chunk_ae.trigger();
    }

    fn set_outcome(&self, outcome: Outcome) {
        *self.outcome.borrow_mut() = Some(outcome);
        self.outcome_ae.trigger();
    }

    async fn wait_chunks(&self, n: usize) {
        loop {
            let have = self.chunks.borrow().len();
            if have >= n {
                return;
            }
            self.chunk_ae.triggered().await;
        }
    }

    async fn wait_outcome(&self) {
        loop {
            if self.outcome.borrow().is_some() {
                return;
            }
            self.outcome_ae.triggered().await;
        }
    }

    fn data(&self) -> Vec<Vec<u8>> {
        self.chunks
            .borrow()
            .iter()
            .map(|c| c.data.clone())
            .collect()
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

/// Waits until no multishot task is left in the ring.
///
/// Once the task is gone, every completion it produced has been dispatched and
/// every buffer it consumed has been returned to the buffer ring.
async fn wait_no_multishot(ring: &Rc<IoUring>) {
    let debouncer = ring.debouncer(0);
    while ring.ring.multishot_tasks.is_not_empty() {
        debouncer.debounce().await;
    }
}

/// Each write produces one chunk and the buffers are reused.
#[test]
fn read() {
    let chunks: &[&[u8]] = &[b"aaaa", b"bbbb", b"cccc", b"dddd", b"eeee", b"ffff"];
    assert!(chunks.len() > NUM_BUFFERS);

    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe { read, write } = new_pipe();
    let reader = Reader::new(&ring, &read, &buffer_ring);
    assert!(reader.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader2 = reader.clone();
    let _fut = eng.spawn("", async move {
        for (idx, chunk) in chunks.iter().enumerate() {
            uapi::write(write.raw(), *chunk).unwrap();
            reader2.wait_chunks(idx + 1).await;
        }
        reader2.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    let expected: Vec<_> = chunks.iter().map(|c| c.to_vec()).collect();
    assert_eq!(reader.data(), expected);
    // The read never stopped on its own.
    assert_eq!(*reader.outcome.borrow(), None);
}

/// A single sqe produces multiple completions within a single ring iteration.
#[test]
fn multiple_completions_per_iteration() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe { read, write } = new_pipe();
    let reader = Reader::new(&ring, &read, &buffer_ring);
    assert!(reader.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader2 = reader.clone();
    let _fut = eng.spawn("", async move {
        // Enough data to fill every buffer in the ring. The kernel drains the
        // pipe before io_uring_enter returns, so all completions are dispatched
        // in the same iteration.
        uapi::write(write.raw(), b"aaaabbbb").unwrap();
        reader2.wait_chunks(NUM_BUFFERS).await;
        reader2.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(reader.data(), vec![b"aaaa".to_vec(), b"bbbb".to_vec()]);
    let chunks = reader.chunks.borrow();
    // Both chunks were produced by the first sqe, in the same iteration. This
    // holds regardless of whether the second chunk arrived in an intermediate
    // or in the final completion.
    assert!(
        chunks.iter().all(|c| c.arm == 1),
        "chunks were produced by multiple sqes: {chunks:?}",
    );
    assert!(
        chunks.iter().all(|c| c.iteration == chunks[0].iteration),
        "chunks were spread across iterations: {chunks:?}",
    );
}

/// Reads larger than a single buffer are split across buffers.
#[test]
fn read_split() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe { read, write } = new_pipe();
    let reader = Reader::new(&ring, &read, &buffer_ring);
    assert!(reader.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader2 = reader.clone();
    let _fut = eng.spawn("", async move {
        uapi::write(write.raw(), b"aaaabb").unwrap();
        reader2.wait_chunks(2).await;
        reader2.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(reader.data(), vec![b"aaaa".to_vec(), b"bb".to_vec()]);
}

/// Closing the write end stops the read.
#[test]
fn eof() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe { read, write } = new_pipe();
    let reader = Reader::new(&ring, &read, &buffer_ring);
    assert!(reader.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader2 = reader.clone();
    let _fut = eng.spawn("", async move {
        uapi::write(write.raw(), b"aaaa").unwrap();
        reader2.wait_chunks(1).await;
        drop(write);
        reader2.wait_outcome().await;
        reader2.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(reader.data(), vec![b"aaaa".to_vec()]);
    assert_eq!(*reader.outcome.borrow(), Some(Outcome::Eof));
}

/// The read stops once the buffer ring runs out of buffers.
#[test]
fn no_buffers() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe { read, write } = new_pipe();
    let reader = Reader::new(&ring, &read, &buffer_ring);
    reader.keep_buffers.set(true);
    assert!(reader.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader2 = reader.clone();
    let _fut = eng.spawn("", async move {
        // One buffer more than the ring can hold.
        uapi::write(write.raw(), b"aaaabbbbcccc").unwrap();
        reader2.wait_outcome().await;
        reader2.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(reader.data(), vec![b"aaaa".to_vec(), b"bbbb".to_vec()]);
    assert_eq!(*reader.outcome.borrow(), Some(Outcome::NoBuffers));
}

/// A read that ran out of buffers can be armed again once buffers are
/// available.
///
/// Running out of buffers does not consume any data, so the read picks up where
/// it left off.
#[test]
fn no_buffers_rearm() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe { read, write } = new_pipe();
    let reader = Reader::new(&ring, &read, &buffer_ring);
    reader.keep_buffers.set(true);
    assert!(reader.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader2 = reader.clone();
    let first_outcome = Rc::new(RefCell::new(None));
    let first_outcome2 = first_outcome.clone();
    let _fut = eng.spawn("", async move {
        // One buffer more than the ring can hold.
        uapi::write(write.raw(), b"aaaabbbbcccc").unwrap();
        reader2.wait_outcome().await;
        // Returning the buffers to the ring is what makes arming the read again
        // meaningful, so the reader cannot do it on its own.
        *first_outcome2.borrow_mut() = reader2.release_buffers();
        reader2.keep_buffers.set(false);
        assert!(reader2.arm());
        reader2.wait_chunks(3).await;
        reader2.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(*first_outcome.borrow(), Some(Outcome::NoBuffers));
    assert_eq!(
        reader.data(),
        vec![b"aaaa".to_vec(), b"bbbb".to_vec(), b"cccc".to_vec()],
    );
    // The data that was left over was not lost.
    assert_eq!(*reader.outcome.borrow(), None);
}

/// Concurrent reads can share a buffer ring.
///
/// The buffers are handed out from the shared pool, so no buffer is ever used
/// by two reads at the same time and a buffer released by one read becomes
/// available to the other.
#[test]
fn shared_buffer_ring() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe {
        read: read1,
        write: write1,
    } = new_pipe();
    let Pipe {
        read: read2,
        write: write2,
    } = new_pipe();
    let reader1 = Reader::new(&ring, &read1, &buffer_ring);
    let reader2 = Reader::new(&ring, &read2, &buffer_ring);
    // Retain the buffers so that both reads hold one at the same time.
    reader1.keep_buffers.set(true);
    reader2.keep_buffers.set(true);
    assert!(reader1.arm());
    assert!(reader2.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader1b = reader1.clone();
    let reader2b = reader2.clone();
    let outcome1 = Rc::new(RefCell::new(None));
    let outcome1b = outcome1.clone();
    let _fut = eng.spawn("", async move {
        // The ring holds exactly one buffer for each of the two reads.
        uapi::write(write1.raw(), b"aaaa").unwrap();
        reader1b.wait_chunks(1).await;
        uapi::write(write2.raw(), b"bbbb").unwrap();
        reader2b.wait_chunks(1).await;
        // The pool is empty, so the first read can no longer get a buffer.
        uapi::write(write1.raw(), b"cccc").unwrap();
        reader1b.wait_outcome().await;
        // Returning the buffer of the first read makes it available to the
        // second one.
        *outcome1b.borrow_mut() = reader1b.release_buffers();
        uapi::write(write2.raw(), b"dddd").unwrap();
        reader2b.wait_chunks(2).await;
        reader2b.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(reader1.data(), vec![b"aaaa".to_vec()]);
    assert_eq!(reader2.data(), vec![b"bbbb".to_vec(), b"dddd".to_vec()]);
    assert_eq!(*outcome1.borrow(), Some(Outcome::NoBuffers));
    let addr1 = reader1.chunks.borrow()[0].addr;
    let chunks2 = reader2.chunks.borrow();
    // Neither read was handed the buffer the other one was holding.
    assert_ne!(addr1, chunks2[0].addr);
    // The buffer released by the first read was reused by the second one.
    assert_eq!(addr1, chunks2[1].addr);
}

/// Buffers of completions that are delivered after the read has been cancelled
/// are returned to the buffer ring.
#[test]
fn cancel_returns_buffers() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe {
        read: read1,
        write: write1,
    } = new_pipe();
    let Pipe {
        read: read2,
        write: write2,
    } = new_pipe();
    let reader1 = Reader::new(&ring, &read1, &buffer_ring);
    let reader2 = Reader::new(&ring, &read2, &buffer_ring);
    reader1.cancel_on_first_chunk.set(true);
    // The second read keeps its buffers so that it runs out of buffers exactly
    // when it has consumed the entire ring.
    reader2.keep_buffers.set(true);
    assert!(reader1.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader1b = reader1.clone();
    let reader2b = reader2.clone();
    let _fut = eng.spawn("", async move {
        // Consumes both buffers. The first completion cancels the read, so the
        // second one is delivered without a callback.
        uapi::write(write1.raw(), b"aaaabbbb").unwrap();
        reader1b.wait_chunks(1).await;
        wait_no_multishot(&ring2).await;
        // If the second buffer had been lost, this read would only see one.
        assert!(reader2b.arm());
        // One buffer more than the ring can hold, so that the read runs out of
        // buffers instead of waiting for more data.
        uapi::write(write2.raw(), b"ccccddddeeee").unwrap();
        reader2b.wait_outcome().await;
        reader2b.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(reader1.data(), vec![b"aaaa".to_vec()]);
    assert_eq!(reader2.data(), vec![b"cccc".to_vec(), b"dddd".to_vec()]);
    assert_eq!(*reader2.outcome.borrow(), Some(Outcome::NoBuffers));
}

/// Reading from a file that cannot be polled fails.
#[test]
fn not_pollable() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let file = Rc::new(uapi::open("/dev/zero", c::O_RDONLY | c::O_CLOEXEC, 0).unwrap());
    let reader = Reader::new(&ring, &file, &buffer_ring);
    assert!(reader.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader2 = reader.clone();
    let _fut = eng.spawn("", async move {
        reader2.wait_outcome().await;
        reader2.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    assert_eq!(reader.data().len(), 0);
    assert_eq!(
        *reader.outcome.borrow(),
        Some(Outcome::Error(OsError(c::EBADFD))),
    );
}

/// The buffers of a buffer ring respect the requested alignment.
#[test]
fn buffer_alignment() {
    const ALIGNMENT: usize = 64;
    // Not a multiple of the alignment, so that the entries have to be padded to
    // keep every buffer aligned.
    const SIZE: usize = 3;
    const NUM: usize = 4;

    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring.create_buffer_ring(NUM, SIZE, ALIGNMENT).unwrap();
    let Pipe { read, write } = new_pipe();
    let reader = Reader::new(&ring, &read, &buffer_ring);
    // Retain the buffers so that every buffer of the ring is used exactly once.
    reader.keep_buffers.set(true);
    assert!(reader.arm());

    let _watchdog = watchdog(&eng, &ring);
    let ring2 = ring.clone();
    let reader2 = reader.clone();
    let _fut = eng.spawn("", async move {
        uapi::write(write.raw(), &[b'a'; NUM * SIZE]).unwrap();
        reader2.wait_chunks(NUM).await;
        reader2.cancel();
        ring2.stop();
    });
    ring.run().unwrap();

    let chunks = reader.chunks.borrow();
    assert_eq!(chunks.len(), NUM);
    for chunk in &*chunks {
        assert_eq!(chunk.data, [b'a'; SIZE]);
        assert_eq!(
            chunk.addr % ALIGNMENT,
            0,
            "buffer {:x} is not aligned to {}",
            chunk.addr,
            ALIGNMENT,
        );
    }
    // Every chunk was delivered in a different buffer.
    let mut addrs: Vec<_> = chunks.iter().map(|c| c.addr).collect();
    addrs.sort_unstable();
    addrs.dedup();
    assert_eq!(addrs.len(), NUM);
}

/// A buffer ring can only be used with the io-uring that created it.
#[test]
fn different_ring() {
    let eng = AsyncEngine::new();
    let ring1 = IoUring::new(&eng, 32).unwrap();
    let ring2 = IoUring::new(&eng, 32).unwrap();
    let buffer_ring = ring1
        .create_buffer_ring(NUM_BUFFERS, BUFFER_SIZE, 1)
        .unwrap();
    let Pipe {
        read,
        write: _write,
    } = new_pipe();
    let reader = Reader::new(&ring1, &read, &buffer_ring);
    let res = ring2.read_multishot(&read, &buffer_ring, reader);
    assert!(matches!(
        res,
        Err(IoUringError::BufferRing(BufferRingError::DifferentRing))
    ));
}
