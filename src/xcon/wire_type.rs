use {
    crate::xcon::{XconError, formatter::Formatter, parser::Parser},
    bstr::{BStr, ByteSlice},
    std::{borrow::Cow, fmt::Debug, rc::Rc},
    uapi::OwnedFd,
};

#[cold]
fn unimplemented() -> ! {
    unimplemented!();
}

pub unsafe trait Message<'a>: Clone + Debug + 'a {
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
    const EXTENSION: Option<usize>;
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

#[derive(Debug, Clone)]
pub(super) struct SendEvent {
    pub propagate: u8,
    pub destination: u32,
    pub event_mask: u32,
}

unsafe impl<'a> Message<'a> for SendEvent {
    type Generic<'b> = SendEvent;
    const IS_POD: bool = false;
    const HAS_FDS: bool = false;

    fn serialize(&self, formatter: &mut Formatter) {
        {
            let propagate_bytes = self.propagate.to_ne_bytes();
            let destination_bytes = self.destination.to_ne_bytes();
            let event_mask_bytes = self.event_mask.to_ne_bytes();
            formatter.write_bytes(&[
                25,
                propagate_bytes[0],
                0,
                0,
                destination_bytes[0],
                destination_bytes[1],
                destination_bytes[2],
                destination_bytes[3],
                event_mask_bytes[0],
                event_mask_bytes[1],
                event_mask_bytes[2],
                event_mask_bytes[3],
            ]);
        }
    }
}
