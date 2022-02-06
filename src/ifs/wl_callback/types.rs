use crate::client::EventFormatter;
use crate::ifs::wl_callback::{WlCallback, DONE};
use crate::object::Object;
use crate::utils::buffd::MsgFormatter;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlCallbackError {}
