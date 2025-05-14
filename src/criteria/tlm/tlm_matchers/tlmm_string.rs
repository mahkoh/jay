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
pub type TlmMatchClass = TlmMatchString<ClassAccess>;
pub type TlmMatchInstance = TlmMatchString<InstanceAccess>;
pub type TlmMatchRole = TlmMatchString<RoleAccess>;
pub type TlmMatchWorkspace = TlmMatchString<WorkspaceAccess>;

pub struct TitleAccess;
pub struct AppIdAccess;
pub struct TagAccess;
pub struct ClassAccess;
pub struct InstanceAccess;
pub struct RoleAccess;
pub struct WorkspaceAccess;

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

impl StringAccess<ToplevelData> for ClassAccess {
    fn with_string(data: &ToplevelData, f: impl FnOnce(&str) -> bool) -> bool {
        if let ToplevelType::XWindow(data) = &data.kind {
            return f(&data.info.class.borrow().as_deref().unwrap_or_default());
        }
        false
    }

    fn nodes(roots: &RootMatchers) -> &TlmRootMatcherMap<TlmMatchString<Self>> {
        &roots.class
    }
}

impl StringAccess<ToplevelData> for InstanceAccess {
    fn with_string(data: &ToplevelData, f: impl FnOnce(&str) -> bool) -> bool {
        if let ToplevelType::XWindow(data) = &data.kind {
            return f(&data.info.instance.borrow().as_deref().unwrap_or_default());
        }
        false
    }

    fn nodes(roots: &RootMatchers) -> &TlmRootMatcherMap<TlmMatchString<Self>> {
        &roots.instance
    }
}

impl StringAccess<ToplevelData> for RoleAccess {
    fn with_string(data: &ToplevelData, f: impl FnOnce(&str) -> bool) -> bool {
        if let ToplevelType::XWindow(data) = &data.kind {
            return f(&data.info.role.borrow().as_deref().unwrap_or_default());
        }
        false
    }

    fn nodes(roots: &RootMatchers) -> &TlmRootMatcherMap<TlmMatchString<Self>> {
        &roots.role
    }
}

impl StringAccess<ToplevelData> for WorkspaceAccess {
    fn with_string(data: &ToplevelData, f: impl FnOnce(&str) -> bool) -> bool {
        if let Some(ws) = data.workspace.get() {
            return f(&ws.name);
        }
        false
    }

    fn nodes(roots: &RootMatchers) -> &TlmRootMatcherMap<TlmMatchString<Self>> {
        &roots.workspace
    }
}
