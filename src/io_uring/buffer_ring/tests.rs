use crate::async_engine::AsyncEngine;
use crate::io_uring::IoUring;
use crate::io_uring::IoUringError;
use crate::io_uring::buffer_ring::BufferRingError;
use std::rc::Rc;

fn ring() -> Rc<IoUring> {
    let eng = AsyncEngine::new();
    IoUring::new(&eng, 32).unwrap()
}

macro_rules! assert_err {
    ($res:expr, $pat:pat $(,)?) => {{
        let res = $res;
        assert!(
            matches!(res, Err(IoUringError::BufferRing($pat))),
            "unexpected result: {:?}",
            res.map(|_| ()),
        );
    }};
}

/// The number of entries is rounded up to a power of two.
#[test]
fn entries_are_rounded_up() {
    let ring = ring();
    for (num, entries) in [(0, 1), (1, 1), (2, 2), (3, 4), (5, 8), (255, 256)] {
        let buffer_ring = ring.create_buffer_ring(num, 1, 1).unwrap();
        assert_eq!(buffer_ring.mask + 1, entries, "num = {num}");
    }
}

/// Entries are padded so that every buffer satisfies the alignment.
#[test]
fn entries_are_padded() {
    let ring = ring();
    let buffer_ring = ring.create_buffer_ring(2, 3, 8).unwrap();
    assert_eq!(buffer_ring.len, 3);
    assert_eq!(buffer_ring.stride, 8);
    assert_eq!(buffer_ring.buf as usize % 8, 0);
    // Sizes that are already a multiple of the alignment are not padded.
    let buffer_ring = ring.create_buffer_ring(2, 16, 8).unwrap();
    assert_eq!(buffer_ring.stride, 16);
}

/// Buffer group ids are allocated in order and released again.
#[test]
fn buffer_group_ids() {
    let ring = ring();
    let first = ring.create_buffer_ring(1, 1, 1).unwrap();
    let second = ring.create_buffer_ring(1, 1, 1).unwrap();
    assert_eq!(first.bgid, 0);
    assert_eq!(second.bgid, 1);
    drop(first);
    // Dropping the buffer ring also unregisters it in the kernel. Otherwise the
    // kernel would reject the id with EEXIST.
    let third = ring.create_buffer_ring(1, 1, 1).unwrap();
    assert_eq!(third.bgid, 0);
}

/// Buffer rings cannot be created on a destroyed io-uring.
#[test]
fn destroyed() {
    let ring = ring();
    ring.stop();
    let res = ring.create_buffer_ring(1, 1, 1);
    assert!(matches!(res, Err(IoUringError::Destroyed)));
}

/// Entries must have a size.
#[test]
fn zero_size() {
    let ring = ring();
    assert_err!(ring.create_buffer_ring(1, 0, 1), BufferRingError::ZeroSize);
}

/// The number of entries must be representable.
#[test]
fn too_many_entries() {
    let ring = ring();
    // Cannot be rounded up to a power of two.
    assert_err!(
        ring.create_buffer_ring(usize::MAX, 1, 1),
        BufferRingError::TooManyEntries,
    );
}

/// The size of an entry must fit in the u32 field of a buffer ring entry.
#[test]
fn size_overflow() {
    let ring = ring();
    assert_err!(
        ring.create_buffer_ring(1, u32::MAX as usize + 1, 1),
        BufferRingError::SizeOverflow,
    );
}

/// The entries must have a valid layout.
#[test]
fn invalid_layout() {
    let ring = ring();
    // The alignment is not a power of two.
    assert_err!(ring.create_buffer_ring(1, 1, 3), BufferRingError::Layout(_),);
    // The entries do not fit in the address space.
    assert_err!(
        ring.create_buffer_ring(1 << (usize::BITS - 2), 1024, 1),
        BufferRingError::Layout(_),
    );
}

/// The allocation of the entries can fail.
#[test]
fn allocation_failure() {
    let ring = ring();
    assert_err!(
        ring.create_buffer_ring(1 << (usize::BITS - 2), 1, 1),
        BufferRingError::Allocation,
    );
}

/// The kernel rejects buffer rings that it cannot address.
#[test]
fn kernel_rejects_ring() {
    let ring = ring();
    // The kernel cannot distinguish a full from an empty ring at this size.
    assert_err!(
        ring.create_buffer_ring(65536, 1, 1),
        BufferRingError::Register(_),
    );
    // The id and the memory were released again.
    let buffer_ring = ring.create_buffer_ring(1, 1, 1).unwrap();
    assert_eq!(buffer_ring.bgid, 0);
}
