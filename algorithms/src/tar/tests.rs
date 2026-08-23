use crate::tar::MAX_SIZE;
use crate::tar::TarWriter;
use isnt::std_1::primitive::IsntSliceExt;
use isnt::std_1::vec::IsntVecExt;
use std::ffi::OsStr;
use std::fs;
use std::io::BufWriter;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;

// A directory below TMPDIR that is removed when the test ends.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "jay-tar-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    // Writes an archive and returns its path.
    fn archive(&self, f: impl FnOnce(&mut TarWriter<'_>)) -> PathBuf {
        let mut buf_writer = BufWriter::new(Vec::new());
        {
            let mut writer = TarWriter::new(&mut buf_writer);
            f(&mut writer);
            writer.finish().unwrap();
        }
        let buf = buf_writer.into_inner().unwrap();
        let path = self.0.join("archive.tar");
        fs::write(&path, &buf).unwrap();
        path
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Runs tar and returns its stdout. tar reports some malformed-archive conditions
// on stderr while still exiting successfully, so both are checked.
fn tar(args: &[&OsStr]) -> Vec<u8> {
    let output = Command::new("tar")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("could not execute tar: {e}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stderr.is_empty(),
        "tar {args:?} exited with {} and wrote {stderr:?}",
        output.status,
    );
    output.stdout
}

// The member names tar reports, in archive order.
fn list(archive: &Path) -> Vec<Vec<u8>> {
    let stdout = tar(&[
        OsStr::new("--quoting-style=literal"),
        OsStr::new("-tf"),
        archive.as_os_str(),
    ]);
    stdout
        .split(|b| *b == b'\n')
        .filter(|l| l.is_not_empty())
        .map(|l| l.to_vec())
        .collect()
}

// The leading type/mode column of `tar -tvf`, e.g. `drwxr-xr-x`.
fn modes(archive: &Path) -> Vec<String> {
    let stdout = tar(&[
        OsStr::new("--quoting-style=literal"),
        OsStr::new("-tvf"),
        archive.as_os_str(),
    ]);
    String::from_utf8(stdout)
        .unwrap()
        .lines()
        .map(|l| l.split_whitespace().next().unwrap().to_string())
        .collect()
}

// Extracts the archive into a fresh directory and returns it.
fn extract(dir: &TmpDir, archive: &Path) -> PathBuf {
    let root = dir.0.join("root");
    fs::create_dir(&root).unwrap();
    tar(&[
        OsStr::new("-xpf"),
        archive.as_os_str(),
        OsStr::new("-C"),
        root.as_os_str(),
    ]);
    root
}

fn path_of(root: &Path, name: &[u8]) -> PathBuf {
    root.join(OsStr::from_bytes(name))
}

#[test]
fn ustar() {
    let dir = TmpDir::new();
    let archive = dir.archive(|w| {
        w.add_dir(b"a").unwrap();
        w.add_reg(b"a/b", b"hello world").unwrap();
        w.add_lnk(b"a/c", b"b").unwrap();
    });
    // add_dir does not append a separator; the type flag marks the directory.
    assert_eq!(list(&archive), [&b"a"[..], b"a/b", b"a/c"]);
    assert_eq!(modes(&archive), ["drwxr-xr-x", "-rw-r--r--", "lrwxrwxrwx"]);
    let root = extract(&dir, &archive);
    assert!(root.join("a").is_dir());
    assert_eq!(fs::read(root.join("a/b")).unwrap(), b"hello world");
    assert_eq!(fs::read_link(root.join("a/c")).unwrap(), Path::new("b"));
    let mode = |p: &str| fs::metadata(root.join(p)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode("a"), 0o755);
    assert_eq!(mode("a/b"), 0o644);
}

// Contents that are not a multiple of the record size must be zero padded so
// that the next header starts on a record boundary.
#[test]
fn contents_padding() {
    let dir = TmpDir::new();
    let sizes = [0usize, 1, 511, 512, 513, 1023, 1024, 1025];
    let contents: Vec<Vec<u8>> = sizes.iter().map(|n| b"x".repeat(*n)).collect();
    let archive = dir.archive(|w| {
        for (i, c) in contents.iter().enumerate() {
            w.add_reg(format!("f{i}").as_bytes(), c).unwrap();
        }
        // Only reachable if every preceding entry left the stream aligned.
        w.add_reg(b"last", b"end").unwrap();
    });
    let root = extract(&dir, &archive);
    for (i, c) in contents.iter().enumerate() {
        assert_eq!(&fs::read(root.join(format!("f{i}"))).unwrap(), c);
    }
    assert_eq!(fs::read(root.join("last")).unwrap(), b"end");
}

// A path that does not fit in the ustar name/prefix fields goes into a pax
// extended header.
#[test]
fn pax_path() {
    let dir = TmpDir::new();
    // One component, longer than the 100 byte name field and with no separator
    // to split on, but still below NAME_MAX so that it can be extracted.
    let name = b"p".repeat(200);
    let archive = dir.archive(|w| {
        w.add_reg(&name, b"hello world").unwrap();
        w.add_reg(b"after", b"still in sync").unwrap();
    });
    assert_eq!(list(&archive), [name.clone(), b"after".to_vec()]);
    let root = extract(&dir, &archive);
    assert_eq!(fs::read(path_of(&root, &name)).unwrap(), b"hello world");
    assert_eq!(fs::read(root.join("after")).unwrap(), b"still in sync");
}

// A symlink target longer than the 100 byte linkname field goes into a pax
// extended header, as does a target combined with a long path.
#[test]
fn pax_linkpath() {
    let dir = TmpDir::new();
    let target = b"t".repeat(200);
    let long = b"l".repeat(200);
    let archive = dir.archive(|w| {
        w.add_lnk(b"short", &target).unwrap();
        w.add_lnk(&long, &target).unwrap();
        w.add_reg(b"after", b"still in sync").unwrap();
    });
    assert_eq!(
        list(&archive),
        [b"short".to_vec(), long.clone(), b"after".to_vec()],
    );
    let root = extract(&dir, &archive);
    let read_link = |p: PathBuf| fs::read_link(p).unwrap().into_os_string().into_vec();
    assert_eq!(read_link(root.join("short")), target);
    assert_eq!(read_link(path_of(&root, &long)), target);
    assert_eq!(fs::read(root.join("after")).unwrap(), b"still in sync");
}

// Contents too large for the ustar size field go into a pax size record. The
// real bound needs an 8 GB slice, so the test drives the bound directly.
#[test]
fn pax_size() {
    let dir = TmpDir::new();
    let big = b"b".repeat(600);
    let archive = dir.archive(|w| {
        w.add_reg_max(b"small", b"abc", 1 << 33).unwrap();
        // Forced over the bound: written with a pax size record and an empty
        // ustar size field.
        w.add_reg_max(b"big", &big, 8).unwrap();
        w.add_reg_max(b"after", b"still in sync", 1 << 33).unwrap();
    });
    assert_eq!(list(&archive), [&b"small"[..], b"big", b"after"]);
    let root = extract(&dir, &archive);
    assert_eq!(fs::read(root.join("small")).unwrap(), b"abc");
    assert_eq!(fs::read(root.join("big")).unwrap(), big);
    assert_eq!(fs::read(root.join("after")).unwrap(), b"still in sync");
}

// MAX_USTAR_SIZE must be the first size that no longer fits, otherwise add_reg
// writes an unterminated or truncated size field and the stream desynchronizes.
#[test]
fn max_ustar_size_fits_size_field() {
    let mut field = [0u8; 12];
    write!(&mut field[..], "{:011o}", MAX_SIZE - 1).unwrap();
    assert_eq!(
        field[11],
        0,
        "size field for {} is not terminated: {:?}",
        MAX_SIZE - 1,
        field,
    );
}

// Paths are split over the ustar name and prefix fields where possible and fall
// back to a pax header where not. Either way tar must report them unchanged.
#[test]
fn path_splitting() {
    let mut names: Vec<Vec<u8>> = Vec::new();
    let mut push = |n: Vec<u8>| names.push(n);
    // Every separator position for lengths around the 100, 155 and 256 bounds.
    for len in (95..=110).chain(150..=170).chain(250..=260) {
        for sep in 1..len - 1 {
            let mut name = vec![b'a'; len];
            name[sep] = b'/';
            push(name);
        }
    }
    let joined = |parts: &[usize]| {
        let mut name = Vec::new();
        for p in parts {
            if name.is_not_empty() {
                name.push(b'/');
            }
            name.extend(std::iter::repeat_n(b'a', *p));
        }
        name
    };
    // Longest name that fits the name field on its own.
    push(joined(&[100]));
    // One byte too long, with no separator to split on.
    push(joined(&[101]));
    // Exactly fills the prefix and name fields.
    push(joined(&[155, 100]));
    // Prefix one byte too long.
    push(joined(&[156, 100]));
    // Name one byte too long.
    push(joined(&[155, 101]));
    // Rightmost separator leaves too long a prefix; an earlier one works.
    push(joined(&[150, 20, 30]));
    // Rightmost separator leaves too long a prefix and the earlier one leaves
    // too long a name.
    push(joined(&[50, 110, 50]));

    let dir = TmpDir::new();
    let archive = dir.archive(|w| {
        for name in &names {
            w.add_reg(name, b"").unwrap();
        }
    });
    let listed = list(&archive);
    assert_eq!(listed.len(), names.len());
    for (name, got) in names.iter().zip(&listed) {
        assert_eq!(
            String::from_utf8_lossy(name),
            String::from_utf8_lossy(got),
            "path of length {} was not preserved",
            name.len(),
        );
    }
}
