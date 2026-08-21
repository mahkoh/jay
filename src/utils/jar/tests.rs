use crate::utils::jar::JarError;
use crate::utils::jar::JarEvent;
use crate::utils::jar::JarReader;
use jay_algorithms::jar::JarWriter;
use std::io::BufWriter;
use uapi::OwnedFd;
use uapi::c;

// An owned version of JarEvent. JarEvent borrows from the reader and therefore
// cannot outlive the call to next.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Dir(Vec<u8>),
    DirUp,
    Reg(Vec<u8>, Vec<u8>),
    Lnk(Vec<u8>, Vec<u8>),
}

impl Event {
    fn write(&self, w: &mut JarWriter<'_>) {
        match self {
            Event::Dir(path) => w.add_dir(path).unwrap(),
            Event::DirUp => w.add_dir_up().unwrap(),
            Event::Reg(path, contents) => w.add_reg(path, contents).unwrap(),
            Event::Lnk(path, linkpath) => w.add_lnk(path, linkpath).unwrap(),
        }
    }
}

fn write_jar(f: impl FnOnce(&mut JarWriter<'_>)) -> Vec<u8> {
    let mut buf_writer = BufWriter::new(Vec::new());
    f(&mut JarWriter::new(&mut buf_writer));
    buf_writer.into_inner().unwrap()
}

// Serializes the events and additionally returns the offset of each record
// boundary, starting with 0 and ending with the length of the archive.
fn write_events(events: &[Event]) -> (Vec<u8>, Vec<usize>) {
    let mut buf = vec![];
    let mut boundaries = vec![0];
    for event in events {
        buf.extend_from_slice(&write_jar(|w| event.write(w)));
        boundaries.push(buf.len());
    }
    (buf, boundaries)
}

fn memfd(contents: &[u8]) -> OwnedFd {
    let fd = uapi::memfd_create("jay-jar-test", c::MFD_CLOEXEC).unwrap();
    let mut pos = 0;
    while pos < contents.len() {
        pos += uapi::write(fd.raw(), &contents[pos..]).unwrap();
    }
    fd
}

fn read_jar(contents: &[u8]) -> Result<Vec<Event>, JarError> {
    let fd = memfd(contents);
    let mut reader = JarReader::new(&fd)?;
    let mut events = vec![];
    while let Some(event) = reader.next()? {
        events.push(match event {
            JarEvent::Dir(path) => Event::Dir(path.to_vec()),
            JarEvent::DirUp => Event::DirUp,
            JarEvent::Reg(path, contents) => Event::Reg(path.to_vec(), contents.to_vec()),
            JarEvent::Lnk(path, linkpath) => Event::Lnk(path.to_vec(), linkpath.to_vec()),
        });
    }
    Ok(events)
}

fn assert_roundtrip(events: &[Event]) {
    let (buf, _) = write_events(events);
    assert_eq!(read_jar(&buf).unwrap(), events);
}

#[test]
fn empty() {
    assert_roundtrip(&[]);
}

#[test]
fn single_events() {
    assert_roundtrip(&[Event::Dir(b"dir".to_vec())]);
    assert_roundtrip(&[Event::DirUp]);
    assert_roundtrip(&[Event::Reg(b"file".to_vec(), b"contents".to_vec())]);
    assert_roundtrip(&[Event::Lnk(b"link".to_vec(), b"target".to_vec())]);
}

#[test]
fn tree() {
    assert_roundtrip(&[
        Event::Dir(b"usr".to_vec()),
        Event::Dir(b"share".to_vec()),
        Event::Reg(b"a.txt".to_vec(), b"a".to_vec()),
        Event::Reg(b"b.txt".to_vec(), b"bb".to_vec()),
        Event::Lnk(b"c.txt".to_vec(), b"a.txt".to_vec()),
        Event::DirUp,
        Event::Dir(b"lib".to_vec()),
        Event::DirUp,
        Event::DirUp,
        Event::Reg(b"root.txt".to_vec(), b"root".to_vec()),
    ]);
}

// The framing is length based, so nothing about the payloads is off limits.
#[test]
fn payloads() {
    assert_roundtrip(&[
        Event::Dir(b"".to_vec()),
        Event::Reg(b"".to_vec(), b"".to_vec()),
        Event::Lnk(b"".to_vec(), b"".to_vec()),
        Event::Reg(b"DURL".to_vec(), b"DURL".to_vec()),
        Event::Reg(b"\x00\xff\n/".to_vec(), b"\x00\xff\n/".to_vec()),
        Event::Lnk(b"\x00".to_vec(), b"\xff".to_vec()),
        // A payload that spans more than one page.
        Event::Reg(b"big".to_vec(), vec![0x5a; 3 * 4096 + 1]),
    ]);
}

#[test]
fn many_events() {
    let events: Vec<_> = (0..1000)
        .map(|i| match i % 4 {
            0 => Event::Dir(format!("dir{i}").into_bytes()),
            1 => Event::Reg(format!("file{i}").into_bytes(), vec![i as u8; i]),
            2 => Event::Lnk(
                format!("link{i}").into_bytes(),
                format!("t{i}").into_bytes(),
            ),
            _ => Event::DirUp,
        })
        .collect();
    assert_roundtrip(&events);
}

// Truncating at a record boundary yields the corresponding prefix of the
// events. Truncating anywhere else must be detected.
#[test]
fn truncated() {
    let events = [
        Event::Dir(b"dir".to_vec()),
        Event::DirUp,
        Event::Reg(b"file".to_vec(), b"contents".to_vec()),
        Event::Lnk(b"link".to_vec(), b"target".to_vec()),
        Event::Reg(b"empty".to_vec(), b"".to_vec()),
    ];
    let (buf, boundaries) = write_events(&events);
    for len in 0..=buf.len() {
        let res = read_jar(&buf[..len]);
        match boundaries.iter().position(|b| *b == len) {
            Some(n) => assert_eq!(res.unwrap(), events[..n], "truncated to {len}"),
            None => assert!(
                matches!(res, Err(JarError::Corrupt)),
                "truncated to {len} did not fail",
            ),
        }
    }
}

#[test]
fn unknown_type() {
    let (mut buf, _) = write_events(&[Event::Dir(b"dir".to_vec())]);
    buf[0] = b'X';
    assert!(matches!(read_jar(&buf), Err(JarError::Corrupt)));
}

// A length that exceeds the remaining data must be rejected without overflowing
// the cursor.
#[test]
fn bogus_length() {
    for len in [u64::MAX, u64::MAX - 8, 1 << 60, 5] {
        let (mut buf, _) = write_events(&[Event::Dir(b"dir".to_vec())]);
        buf[1..9].copy_from_slice(&len.to_le_bytes());
        assert!(matches!(read_jar(&buf), Err(JarError::Corrupt)), "{len}");
    }
}
