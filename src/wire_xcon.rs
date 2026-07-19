#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unused_assignments,
    clippy::mixed_read_write_in_expression,
    clippy::double_parens,
    clippy::unnecessary_cast
)]

use crate::xcon::Formatter;
use crate::xcon::Message;
use crate::xcon::Parser;
use crate::xcon::Request;
use crate::xcon::XEvent;
use crate::xcon::XconError;
use bstr::BStr;
use std::borrow::Cow;
use std::rc::Rc;
use uapi::OwnedFd;

include!(concat!(env!("OUT_DIR"), "/wire_xcon.rs"));
