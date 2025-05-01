use crate::{
    criteria::{
        crit_matchers::critm_string::{CritMatchString, StringAccess},
        tlm::{RootMatchers, TlmRootMatcherMap},
    },
    tree::{ToplevelData, ToplevelType},
};

pub type TlmMatchString<T> = CritMatchString<ToplevelData, T>;

pub type TlmMatchTitle = TlmMatchString<TitleAccess>;
pub type TlmMatchAppId = TlmMatchString<AppIdAccess>;
pub type TlmMatchTag = TlmMatchString<TagAccess>;

pub struct TitleAccess;
pub struct AppIdAccess;
pub struct TagAccess;

impl StringAccess<ToplevelData> for TitleAccess {
    fn with_string(data: &ToplevelData, f: impl FnOnce(&str) -> bool) -> bool {
        f(&data.title.borrow())
    }

    fn nodes(roots: &RootMatchers) -> &TlmRootMatcherMap<TlmMatchString<Self>> {
        &roots.title
    }
}

impl StringAccess<ToplevelData> for AppIdAccess {
    fn with_string(data: &ToplevelData, f: impl FnOnce(&str) -> bool) -> bool {
        f(&data.app_id.borrow())
    }

    fn nodes(roots: &RootMatchers) -> &TlmRootMatcherMap<TlmMatchString<Self>> {
        &roots.app_id
    }
}

impl StringAccess<ToplevelData> for TagAccess {
    fn with_string(data: &ToplevelData, f: impl FnOnce(&str) -> bool) -> bool {
        if let ToplevelType::XdgToplevel(data) = &data.kind {
            return f(&data.tag.borrow());
        }
        false
    }

    fn nodes(roots: &RootMatchers) -> &TlmRootMatcherMap<TlmMatchString<Self>> {
        &roots.tag
    }
}
