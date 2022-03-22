use crate::xcon::formatter::Formatter;
use crate::xcon::parser::Parser;
use crate::xcon::XconError;
use bstr::{BStr, ByteSlice};
use std::borrow::Cow;
use std::rc::Rc;
use uapi::OwnedFd;

#[cold]
fn unimplemented() -> ! {
    unimplemented!();
}

pub unsafe trait Message<'a>: Clone + 'a {
    type Generic<'b>: Message<'b>;
    const IS_POD: bool;
    const HAS_FDS: bool;

    fn serialize(&self, formatter: &mut Formatter) {
        let _ = formatter;
        unimplemented()
    }

    fn deserialize(parser: &mut Parser<'a>) -> Result<Self, XconError> {
        let _ = parser;
        unimplemented()
    }
}

pub trait Request<'a>: Message<'a> {
    type Reply: Message<'static>;
    const EXTENSION: Option<usize>;
    const IS_VOID: bool;
}

pub trait XEvent<'a>: Message<'a> {
    const OPCODE: u16;
}

macro_rules! simple {
    ($ty:ty) => {
        unsafe impl Message<'_> for $ty {
            type Generic<'b> = $ty;
            const IS_POD: bool = true;
            const HAS_FDS: bool = false;

            fn serialize(&self, formatter: &mut Formatter) {
                formatter.write_packed(self);
            }

            fn deserialize(parser: &mut Parser<'_>) -> Result<Self, XconError> {
                parser.read_pod()
            }
        }
    };
}

simple!(u8);
simple!(i8);
simple!(u16);
simple!(i16);
simple!(u32);
simple!(i32);
simple!(u64);
simple!(i64);

unsafe impl<'a> Message<'a> for () {
    type Generic<'b> = ();
    const IS_POD: bool = false;
    const HAS_FDS: bool = false;
}

unsafe impl<'a> Message<'a> for &'a BStr {
    type Generic<'b> = &'b BStr;
    const IS_POD: bool = true;
    const HAS_FDS: bool = false;

    fn serialize(&self, formatter: &mut Formatter) {
        formatter.write_packed(self.as_bytes())
    }
}

unsafe impl<'a, T: Message<'a>> Message<'a> for &'a [T] {
    type Generic<'b> = &'b [T::Generic<'b>];
    const IS_POD: bool = false;
    const HAS_FDS: bool = false;

    fn serialize(&self, formatter: &mut Formatter) {
        formatter.write_list(self);
    }
}

unsafe impl<'a, T> Message<'a> for Cow<'a, [T]>
where
    T: Message<'a>,
{
    type Generic<'b> = Cow<'b, [T::Generic<'b>]>;
    const IS_POD: bool = false;
    const HAS_FDS: bool = false;

    fn serialize(&self, formatter: &mut Formatter) {
        formatter.write_list(self);
    }
}

unsafe impl<'a> Message<'a> for Rc<OwnedFd> {
    type Generic<'b> = Rc<OwnedFd>;
    const IS_POD: bool = false;
    const HAS_FDS: bool = true;

    fn serialize(&self, formatter: &mut Formatter) {
        formatter.add_fd(self);
    }

    fn deserialize(parser: &mut Parser<'a>) -> Result<Self, XconError> {
        parser.read_fd()
    }
}
