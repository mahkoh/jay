use crate::async_engine::AsyncEngine;
use crate::io_uring::IoUring;
use crate::utils::cross_process_ring_buffer::Cprb;
use crate::utils::cross_process_ring_buffer::CprbRead;
use crate::utils::cross_process_ring_buffer::CprbWrite;
use crate::wheel::Wheel;

struct TestCprb;

impl Cprb for TestCprb {
    type Slot = u64;
    type Data = [u64; 16];
}

const NUM_SLOTS: usize = 4;

type Writer = CprbWrite<TestCprb, NUM_SLOTS>;
type Reader = CprbRead<TestCprb, NUM_SLOTS>;

fn pair() -> (Writer, Reader) {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let writer = CprbWrite::new(&ring).unwrap();
    let reader = CprbRead::new(&ring, writer.memfd()).unwrap();
    (writer, reader)
}

fn write(writer: &Writer, val: u64) {
    let msg = writer.acquire().expect("acquire failed");
    unsafe { msg.slot.write(val) };
    writer.commit(0);
}

fn read_msg(reader: &mut Reader) -> (u64, u64) {
    let msg = reader.acquire().expect("no message");
    let val = unsafe { msg.slot.read() };
    (val, msg.missed)
}

fn read(reader: &mut Reader) -> u64 {
    let (val, missed) = read_msg(reader);
    assert_eq!(missed, 0);
    val
}

#[test]
fn empty() {
    let (_writer, mut reader) = pair();
    assert!(reader.acquire().is_none());
}

#[test]
fn single_message() {
    let (writer, mut reader) = pair();
    write(&writer, 0x42);
    {
        let msg = reader.acquire().unwrap();
        assert_eq!(msg.missed, 0);
        assert_eq!(unsafe { msg.slot.read() }, 0x42);
    }
    assert!(reader.acquire().is_none());
}

#[test]
fn fifo() {
    let (writer, mut reader) = pair();
    for i in 0..NUM_SLOTS as u64 {
        write(&writer, i);
    }
    for i in 0..NUM_SLOTS as u64 {
        assert_eq!(read(&mut reader), i);
    }
    assert!(reader.acquire().is_none());
}

#[test]
fn full() {
    let (writer, mut reader) = pair();
    for i in 0..NUM_SLOTS as u64 {
        write(&writer, i);
    }
    assert!(writer.acquire().is_none());
    assert_eq!(read(&mut reader), 0);
    write(&writer, 99);
    assert!(writer.acquire().is_none());
    assert_eq!(read(&mut reader), 1);
    assert_eq!(read(&mut reader), 2);
    assert_eq!(read(&mut reader), 3);
    // The failed acquire above consumed a serial, so the reader observes a gap.
    let (val, missed) = read_msg(&mut reader);
    assert_eq!(val, 99);
    assert_eq!(missed, 1);
    assert!(reader.acquire().is_none());
}

#[test]
fn uncommitted_acquire_is_skipped() {
    let (writer, mut reader) = pair();
    {
        let _msg = writer.acquire().unwrap();
    }
    write(&writer, 7);
    let msg = reader.acquire().unwrap();
    assert_eq!(msg.missed, 1);
    assert_eq!(unsafe { msg.slot.read() }, 7);
}

#[test]
fn wraparound() {
    let (writer, mut reader) = pair();
    for i in 0..(3 * NUM_SLOTS + 1) as u64 {
        write(&writer, i);
        assert_eq!(read(&mut reader), i);
    }
    assert!(reader.acquire().is_none());
}

#[test]
fn held_message_blocks_writer() {
    let (writer, mut reader) = pair();
    for i in 0..NUM_SLOTS as u64 {
        write(&writer, 10 + i);
    }
    let msg = reader.acquire().unwrap();
    assert_eq!(unsafe { msg.slot.read() }, 10);
    // The slot is only released when the message is dropped.
    assert!(writer.acquire().is_none());
    drop(msg);
    write(&writer, 99);
    assert!(writer.acquire().is_none());
    for i in 1..NUM_SLOTS as u64 {
        assert_eq!(read(&mut reader), 10 + i);
    }
    // The failed acquire above consumed a serial, so the reader observes a gap.
    let (val, missed) = read_msg(&mut reader);
    assert_eq!(val, 99);
    assert_eq!(missed, 1);
}

#[test]
fn data_offsets() {
    let (writer, mut reader) = pair();
    {
        let msg = writer.acquire().unwrap();
        unsafe { msg.slot.write(1) };
        writer.commit(5);
    }
    {
        let msg = writer.acquire().unwrap();
        unsafe { msg.slot.write(2) };
        writer.commit(9);
    }
    {
        let msg = writer.acquire().unwrap();
        assert_eq!(msg.write, 9);
        assert_eq!(msg.read, 0);
    }
    assert_eq!(read(&mut reader), 1);
    {
        let msg = writer.acquire().unwrap();
        assert_eq!(msg.read, 5);
    }
    assert_eq!(read(&mut reader), 2);
    {
        let msg = writer.acquire().unwrap();
        assert_eq!(msg.read, 9);
    }
}

#[test]
fn data_is_shared_between_mappings() {
    let (writer, reader) = pair();
    unsafe { writer.data().cast::<u64>().add(3).write(0xdead_beef) };
    assert_eq!(
        unsafe { reader.data().cast::<u64>().add(3).read() },
        0xdead_beef
    );
}

#[test]
fn memfd_cannot_shrink() {
    let (writer, _reader) = pair();
    assert!(uapi::ftruncate(writer.memfd().raw(), 0).is_err());
}

#[test]
fn available_notifies_reader() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let wheel = Wheel::new(&eng, &ring).unwrap();
    let writer: Writer = CprbWrite::new(&ring).unwrap();
    let mut reader: Reader = CprbRead::new(&ring, writer.memfd()).unwrap();
    let available = reader.available();
    let ring2 = ring.clone();
    let _reader = eng.spawn("reader", async move {
        available.available().await.unwrap();
        let msg = reader.acquire().unwrap();
        assert_eq!(unsafe { msg.slot.read() }, 1234);
        ring2.stop();
    });
    let _writer = eng.spawn("writer", async move {
        wheel.timeout(20).await.unwrap();
        write(&writer, 1234);
    });
    ring.run().unwrap();
}
