use anyhow::Result;
use indexmap::IndexMap;
use std::cell::Cell;
use std::cell::RefCell;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fs::File;
use std::io;
use std::io::BufWriter;
use std::io::Write;

thread_local! {
    static TABLE: RefCell<Inner> = Default::default();
    static GENERATED: Cell<bool> = const { Cell::new(false) };
}

struct Inner {
    table: IndexMap<&'static str, usize>,
    len: usize,
    longest: usize,
}

#[derive(Copy, Clone)]
pub struct Interned {
    off: usize,
    s: &'static str,
}

impl From<&str> for Interned {
    fn from(value: &str) -> Self {
        intern(value)
    }
}

impl Debug for Interned {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.s, f)
    }
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            table: Default::default(),
            len: 1,
            longest: 0,
        }
    }
}

pub fn main(open: impl Fn(&str) -> io::Result<BufWriter<File>>) -> Result<()> {
    GENERATED.set(true);
    TABLE.with(|t| {
        let inner = &*t.borrow();
        let mut w = open("str_table.rs")?;
        writeln!(w, "pub type Offset = {};", len_to_ty(inner.len))?;
        writeln!(w, "pub type Length = {};", len_to_ty(inner.longest))?;
        writeln!(w)?;
        writeln!(w, "pub static STR: &str = include_str!(\"str_table.txt\");")?;
        let mut w = open("str_table.txt")?;
        w.write_all(b" ")?;
        for s in inner.table.keys() {
            w.write_all(s.as_bytes())?;
        }
        Ok(())
    })
}

pub fn intern(s: &str) -> Interned {
    assert!(!GENERATED.get());
    TABLE.with(|t| {
        let inner = &mut *t.borrow_mut();
        if let Some((&s, &off)) = inner.table.get_key_value(s) {
            return Interned { off, s };
        }
        let ret = Interned {
            off: inner.len,
            s: s.to_string().leak(),
        };
        inner.len += ret.s.len();
        inner.table.insert(ret.s, ret.off);
        inner.longest = inner.longest.max(ret.s.len());
        ret
    })
}

fn len_to_ty(len: usize) -> &'static str {
    const U08_MAX: usize = u8::MAX as usize;
    const U16_MAX: usize = u16::MAX as usize;
    const U32_MAX: usize = u32::MAX as usize;
    #[expect(clippy::match_overlapping_arm)]
    match len {
        ..=U08_MAX => "u8",
        ..=U16_MAX => "u16",
        ..=U32_MAX => "u32",
        _ => unreachable!(),
    }
}

impl Interned {
    pub fn raw(self) -> &'static str {
        self.s
    }
}

impl Display for Interned {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "const {{ StrAccess::new({}, {}) }}",
            self.off,
            self.s.len(),
        )
    }
}
