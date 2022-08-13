#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unused_assignments,
    clippy::mixed_read_write_in_expression,
    clippy::double_parens,
    clippy::unnecessary_cast
)]

use {
    crate::xcon::{Formatter, Message, Parser, Request, XEvent, XconError},
    bstr::BStr,
    std::{borrow::Cow, rc::Rc},
    uapi::OwnedFd,
};

include!(concat!(env!("OUT_DIR"), "/wire_xcon.rs"));
