use crate::utils::str_table::generated::Length;
use crate::utils::str_table::generated::Offset;
use crate::utils::str_table::generated::STR;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::num::NonZero;
use std::ops::Deref;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/str_table.rs"));
}

#[repr(Rust, packed)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct StrAccess {
    off: NonZero<Offset>,
    len: Length,
}

static_assertions::const_assert_eq!(size_of::<StrAccess>(), size_of::<Option<StrAccess>>());

impl StrAccess {
    pub const fn new(off: Offset, len: Length) -> Self {
        assert!(off as usize <= STR.len());
        assert!(len as usize <= STR.len() - off as usize);
        assert!(STR.is_char_boundary(off as usize));
        assert!(STR.is_char_boundary(off as usize + len as usize));
        Self {
            off: NonZero::new(off).unwrap(),
            len,
        }
    }

    pub fn get(self) -> &'static str {
        unsafe {
            str::from_utf8_unchecked(std::slice::from_raw_parts(
                STR.as_ptr().add(self.off.get() as usize),
                self.len as usize,
            ))
        }
    }
}

impl Deref for StrAccess {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl Debug for StrAccess {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.get(), f)
    }
}

impl Display for StrAccess {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.get(), f)
    }
}

pub trait OptionStrAccessExt {
    fn get(self) -> Option<&'static str>;
}

impl OptionStrAccessExt for Option<StrAccess> {
    fn get(self) -> Option<&'static str> {
        self.map(|v| v.get())
    }
}
