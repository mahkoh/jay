#![allow(unused_macros)]

#[derive(Copy, Clone)]
pub struct ZoneName;

#[derive(Copy, Clone)]
pub struct FrameName;

impl FrameName {
    pub fn get(_name: &str) -> Self {
        Self
    }
}

macro_rules! create_zone_name {
    ($($tt:tt)*) => {
        crate::tracy::ZoneName
    };
}

macro_rules! dynamic_raii_zone {
    ($name:expr) => {};
}

macro_rules! dynamic_zone {
    ($name:expr) => {};
}

macro_rules! raii_zone {
    ($($tt:tt)*) => {
        ()
    };
}

macro_rules! zone {
    ($($tt:tt)*) => {};
}

macro_rules! raii_frame {
    ($name:expr) => {
        ()
    };
}

macro_rules! frame {
    ($name:expr) => {};
}

pub fn enable_profiler() {
    // nothing
}
