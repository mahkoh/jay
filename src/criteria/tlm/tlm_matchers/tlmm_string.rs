use crate::{
    criteria::{
        crit_matchers::critm_string::{CritMatchString, StringAccess},
        tlm::{RootMatchers, TlmRootMatcherMap},
    },
    tree::ToplevelData,
};

pub type TlmMatchString<T> = CritMatchString<ToplevelData, T>;

pub type TlmMatchTitle = TlmMatchString<TitleAccess>;

pub struct TitleAccess;

impl StringAccess<ToplevelData> for TitleAccess {
    fn with_string(data: &ToplevelData, f: impl FnOnce(&str) -> bool) -> bool {
        f(&data.title.borrow())
    }

    fn nodes(roots: &RootMatchers) -> &TlmRootMatcherMap<TlmMatchString<Self>> {
        &roots.title
    }
}
