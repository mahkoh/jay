use crate::jar::JarError;
use crate::jar::JarEvent;
use crate::jar::JarReader;
use crate::jar::JarWriter;
use std::collections::HashSet;
use std::io::BufWriter;
use std::io::Cursor;
use uapi::OwnedFd;
use uapi::c;

// An owned version of JarEvent. JarEvent borrows from the reader and therefore
// cannot outlive the call to next.
//
// The writer stores an id for every regular file whereas the reader reports one
// only if a hard link refers to it. The Some case therefore doubles as the
// expected output: a regular file written with None gets an id that no hard
// link uses, which is exactly what makes the reader drop it again.
#[derive(Debug, Eq, PartialEq)]
enum Event {
    Dir(Vec<u8>),
    DirUp,
    Reg(Vec<u8>, Option<u64>, Vec<u8>),
    Lnk(Vec<u8>, Vec<u8>),
    Hrd(Vec<u8>, u64),
}

fn write_events(events: &[Event]) -> Vec<u8> {
    let linked: HashSet<u64> = events
        .iter()
        .filter_map(|e| match e {
            Event::Hrd(_, unique) => Some(*unique),
            _ => None,
        })
        .collect();
    let mut unlinked = (0u64..).filter(|i| !linked.contains(i));
    let mut buf_writer = BufWriter::new(Cursor::new(Vec::new()));
    let mut w = JarWriter::new(&mut buf_writer).unwrap();
    for event in events {
        match event {
            Event::Dir(path) => w.add_dir(path).unwrap(),
            Event::DirUp => w.add_dir_up().unwrap(),
            Event::Reg(path, unique, contents) => {
                let unique = match unique {
                    // An id that no hard link refers to would come back as
                    // None, so such an event could not round trip.
                    Some(unique) => {
                        assert!(linked.contains(unique), "{unique} is not hard linked");
                        *unique
                    }
                    None => unlinked.next().unwrap(),
                };
                w.add_reg(path, unique, contents).unwrap()
            }
            Event::Lnk(path, linkpath) => w.add_lnk(path, linkpath).unwrap(),
            Event::Hrd(path, unique) => w.add_hrd(path, *unique).unwrap(),
        }
    }
    w.finish().unwrap();
    buf_writer.into_inner().unwrap().into_inner()
}

// The header stores the offset of the hard link set, which is where the records
// end.
fn records_len(archive: &[u8]) -> usize {
    u64::from_le_bytes(archive[..8].try_into().unwrap()) as usize
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
            JarEvent::Reg(path, unique, contents) => {
                Event::Reg(path.to_vec(), unique, contents.to_vec())
            }
            JarEvent::Lnk(path, linkpath) => Event::Lnk(path.to_vec(), linkpath.to_vec()),
            JarEvent::Hrd(path, unique) => Event::Hrd(path.to_vec(), unique),
        });
    }
    Ok(events)
}

fn assert_roundtrip(events: &[Event]) {
    let buf = write_events(events);
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
    assert_roundtrip(&[Event::Reg(b"file".to_vec(), None, b"contents".to_vec())]);
    assert_roundtrip(&[Event::Lnk(b"link".to_vec(), b"target".to_vec())]);
    assert_roundtrip(&[Event::Hrd(b"hard".to_vec(), 1)]);
}

#[test]
fn tree() {
    assert_roundtrip(&[
        Event::Dir(b"usr".to_vec()),
        Event::Dir(b"share".to_vec()),
        Event::Reg(b"a.txt".to_vec(), None, b"a".to_vec()),
        Event::Reg(b"b.txt".to_vec(), None, b"bb".to_vec()),
        Event::Lnk(b"c.txt".to_vec(), b"a.txt".to_vec()),
        Event::DirUp,
        Event::Dir(b"lib".to_vec()),
        Event::DirUp,
        Event::DirUp,
        Event::Reg(b"root.txt".to_vec(), None, b"root".to_vec()),
    ]);
}

// The writer stores an id for every regular file but the reader reports it only
// for the files that a hard link refers to.
#[test]
fn hard_links() {
    assert_roundtrip(&[
        Event::Reg(b"a".to_vec(), Some(1), b"a".to_vec()),
        Event::Hrd(b"b".to_vec(), 1),
        Event::Hrd(b"c".to_vec(), 1),
        Event::Reg(b"d".to_vec(), None, b"d".to_vec()),
    ]);
}

// The hard link set is written after the records, so the reader knows about a
// link before it reaches the file the link refers to.
#[test]
fn hard_link_before_target() {
    assert_roundtrip(&[
        Event::Hrd(b"b".to_vec(), 1),
        Event::Reg(b"a".to_vec(), Some(1), b"a".to_vec()),
    ]);
}

// The framing is length based, so nothing about the payloads is off limits.
#[test]
fn payloads() {
    assert_roundtrip(&[
        Event::Dir(b"".to_vec()),
        Event::Reg(b"".to_vec(), None, b"".to_vec()),
        Event::Lnk(b"".to_vec(), b"".to_vec()),
        Event::Hrd(b"".to_vec(), 0),
        Event::Reg(b"DUHLR".to_vec(), None, b"DUHLR".to_vec()),
        Event::Reg(b"\x00\xff\n/".to_vec(), None, b"\x00\xff\n/".to_vec()),
        Event::Lnk(b"\x00".to_vec(), b"\xff".to_vec()),
        // A payload that spans more than one page.
        Event::Reg(b"big".to_vec(), None, vec![0x5a; 3 * 4096 + 1]),
    ]);
}

#[test]
fn many_events() {
    let events: Vec<_> = (0..1000)
        .map(|i| match i % 5 {
            0 => Event::Dir(format!("dir{i}").into_bytes()),
            1 => Event::Reg(
                format!("file{i}").into_bytes(),
                Some(i as u64),
                vec![i as u8; i],
            ),
            2 => Event::Lnk(
                format!("link{i}").into_bytes(),
                format!("t{i}").into_bytes(),
            ),
            // Refers back to the regular file two events earlier.
            3 => Event::Hrd(format!("hard{i}").into_bytes(), (i - 2) as u64),
            _ => Event::DirUp,
        })
        .collect();
    assert_roundtrip(&events);
}

// The header stores where the records end, so every truncation of the archive
// is detected. A bare record stream would instead accept a cut at a record
// boundary as a shorter archive.
#[test]
fn truncated() {
    let events = [
        Event::Dir(b"dir".to_vec()),
        Event::DirUp,
        Event::Reg(b"file".to_vec(), None, b"contents".to_vec()),
        Event::Lnk(b"link".to_vec(), b"target".to_vec()),
        Event::Reg(b"empty".to_vec(), None, b"".to_vec()),
    ];
    let buf = write_events(&events);
    // No hard links, so the archive ends where the records end.
    assert_eq!(records_len(&buf), buf.len());
    assert_eq!(read_jar(&buf).unwrap(), events);
    for len in 1..buf.len() {
        assert!(
            matches!(read_jar(&buf[..len]), Err(JarError::Corrupt)),
            "truncated to {len}",
        );
    }
}

// Cutting into the hard link set is detected only when it leaves a partial id.
// A cut on an id boundary keeps the records readable and drops the ids behind
// it, so the files they belong to stop being reported as hard linked.
#[test]
fn truncated_hard_link_set() {
    let events = [
        Event::Reg(b"a".to_vec(), Some(1), b"a".to_vec()),
        Event::Hrd(b"b".to_vec(), 1),
    ];
    let buf = write_events(&events);
    let records = records_len(&buf);
    assert_eq!(buf.len() - records, 8);
    for len in records + 1..buf.len() {
        assert!(
            matches!(read_jar(&buf[..len]), Err(JarError::Corrupt)),
            "truncated to {len}",
        );
    }
    assert_eq!(
        read_jar(&buf[..records]).unwrap(),
        [
            Event::Reg(b"a".to_vec(), None, b"a".to_vec()),
            Event::Hrd(b"b".to_vec(), 1),
        ],
    );
}

#[test]
fn unknown_type() {
    let mut buf = write_events(&[Event::Dir(b"dir".to_vec())]);
    buf[8] = b'X';
    assert!(matches!(read_jar(&buf), Err(JarError::Corrupt)));
}

// A length that exceeds the remaining data must be rejected without overflowing
// the cursor.
#[test]
fn bogus_length() {
    for len in [u64::MAX, u64::MAX - 8, 1 << 60, 5] {
        let mut buf = write_events(&[Event::Dir(b"dir".to_vec())]);
        buf[9..17].copy_from_slice(&len.to_le_bytes());
        assert!(matches!(read_jar(&buf), Err(JarError::Corrupt)), "{len}");
    }
}

// The offset of the hard link set must leave room for the header and must not
// point past the end of the archive, and what follows it must be a whole number
// of ids. An offset below the header size used to make the cursor length
// underflow, after which the reader ran off the end of the mapping.
#[test]
fn bogus_hard_link_set_offset() {
    let events = [
        Event::Reg(b"file".to_vec(), Some(1), b"abcdef".to_vec()),
        Event::Hrd(b"link".to_vec(), 1),
    ];
    let mut buf = write_events(&events);
    // The whole number of ids check passes for an offset of 0 only if the
    // archive itself is a multiple of the id size, so keep it one.
    assert_eq!(buf.len() % 8, 0);
    let total = buf.len() as u64;
    for offset in [0, 1, 7, 9, total - 1, total + 1, u64::MAX] {
        buf[..8].copy_from_slice(&offset.to_le_bytes());
        assert!(
            matches!(read_jar(&buf), Err(JarError::Corrupt)),
            "offset {offset}",
        );
    }
}
