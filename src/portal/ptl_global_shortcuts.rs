use crate::dbus::DbusObject;
use crate::dbus::DictEntry;
use crate::dbus::DynamicType;
use crate::dbus::ObjectPath;
use crate::dbus::PendingReply;
use crate::dbus::prelude::Variant;
use crate::portal::DESKTOP_PATH;
use crate::portal::PORTAL_SUCCESS;
use crate::portal::PortalState;
use crate::portal::ptl_display::PortalDisplay;
use crate::portal::ptl_display::PortalDisplayId;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::hash_map_ext::HashMapExt;
use crate::wire_dbus::org;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::Activated;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::BindShortcuts;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::BindShortcutsReply;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::ConfigureShortcuts;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::ConfigureShortcutsReply;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::CreateSession;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::CreateSessionReply;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::Deactivated;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::ListShortcuts;
use crate::wire_dbus::org::freedesktop::impl_::portal::global_shortcuts::ListShortcutsReply;
use crate::wire_dbus::org::freedesktop::impl_::portal::session::CloseReply as SessionCloseReply;
use crate::wire_dbus::org::freedesktop::impl_::portal::session::Closed;
use crate::wl_usr::usr_ifs::usr_hyprland_global_shortcut_v1::UsrHyprlandGlobalShortcutV1;
use crate::wl_usr::usr_ifs::usr_hyprland_global_shortcut_v1::UsrHyprlandGlobalShortcutV1Owner;
use crate::wl_usr::usr_ifs::usr_hyprland_global_shortcuts_manager_v1::UsrHyprlandGlobalShortcutsManagerV1;
use std::borrow::Cow;
use std::cell::RefCell;
use std::mem;
use std::ops::Deref;
use std::rc::Rc;
use std::rc::Weak;

pub struct GlobalShortcutsSession {
    state: Rc<PortalState>,
    app: String,
    session_obj: DbusObject,
    shortcuts: RefCell<Vec<Rc<BoundShortcut>>>,
}

struct BoundShortcut {
    session: Weak<GlobalShortcutsSession>,
    shortcut_id: String,
    description: String,
    preferred_trigger: String,
    trigger_description: String,
    displays: CopyHashMap<PortalDisplayId, Rc<UsrHyprlandGlobalShortcutV1>>,
}

impl BoundShortcut {
    fn register_on_display(
        self: &Rc<Self>,
        dpy: &PortalDisplay,
        gs: &UsrHyprlandGlobalShortcutsManagerV1,
        app_id: &str,
    ) {
        if self.displays.contains(&dpy.id) {
            return;
        }
        let shortcut = gs.register_shortcut(
            &self.shortcut_id,
            app_id,
            &self.description,
            &self.preferred_trigger,
        );
        shortcut.owner.set(Some(self.clone()));
        self.displays.set(dpy.id, shortcut);
    }

    fn destroy_all(&self) {
        for shortcut in self.displays.lock().drain_values() {
            shortcut.con.remove_obj(shortcut.deref());
        }
    }
}

impl UsrHyprlandGlobalShortcutV1Owner for BoundShortcut {
    fn pressed(&self, timestamp_ms: u64) {
        let Some(session) = self.session.upgrade() else {
            return;
        };
        session.state.dbus.emit_signal(
            DESKTOP_PATH,
            &Activated {
                session_handle: ObjectPath(session.session_obj.path().into()),
                shortcut_id: self.shortcut_id.as_str().into(),
                timestamp: timestamp_ms,
                options: Default::default(),
            },
        );
    }

    fn released(&self, timestamp_ms: u64) {
        let Some(session) = self.session.upgrade() else {
            return;
        };
        session.state.dbus.emit_signal(
            DESKTOP_PATH,
            &Deactivated {
                session_handle: ObjectPath(session.session_obj.path().into()),
                shortcut_id: self.shortcut_id.as_str().into(),
                timestamp: timestamp_ms,
                options: Default::default(),
            },
        );
    }
}

impl GlobalShortcutsSession {
    fn kill(&self) {
        self.session_obj.emit_signal(&Closed);
        self.state
            .global_shortcuts_sessions
            .remove(self.session_obj.path());
        let shortcuts = mem::take(&mut *self.shortcuts.borrow_mut());
        for shortcut in shortcuts {
            shortcut.destroy_all();
        }
    }
}

pub(super) fn register_display_shortcuts(dpy: &PortalDisplay) {
    let Some(gs) = dpy.global_shortcuts.get() else {
        return;
    };
    for session in dpy.state.global_shortcuts_sessions.lock().values() {
        for shortcut in session.shortcuts.borrow().iter() {
            shortcut.register_on_display(dpy, &gs, &session.app);
        }
    }
}

pub(super) fn remove_display_shortcuts(state: &PortalState, dpy_id: PortalDisplayId) {
    for session in state.global_shortcuts_sessions.lock().values() {
        for shortcut in session.shortcuts.borrow().iter() {
            if let Some(sc) = shortcut.displays.remove(&dpy_id) {
                sc.owner.take();
            }
        }
    }
}

pub(super) fn add_global_shortcuts_dbus_members(state_: &Rc<PortalState>, object: &DbusObject) {
    use org::freedesktop::impl_::portal::global_shortcuts::*;
    let state = state_.clone();
    object.add_method::<CreateSession<'_>, _>(move |req, pr| {
        dbus_create_session(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<BindShortcuts<'_>, _>(move |req, pr| {
        dbus_bind_shortcuts(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<ListShortcuts<'_>, _>(move |req, pr| {
        dbus_list_shortcuts(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<ConfigureShortcuts<'_>, _>(move |req, pr| {
        dbus_configure_shortcuts(&state, req, pr);
    });
    object.set_property::<version>(Variant::U32(2));
}

fn dbus_create_session(
    state: &Rc<PortalState>,
    req: CreateSession<'_>,
    reply: PendingReply<CreateSessionReply<'static>>,
) {
    log::info!("Create global shortcuts session {req:#?}");
    if state
        .global_shortcuts_sessions
        .contains(req.session_handle.0.deref())
    {
        reply.err("Session already exists");
        return;
    }
    let Ok(obj) = state.dbus.add_object(req.session_handle.0.to_string()) else {
        reply.err("Session path is not unique");
        return;
    };
    let session = Rc::new(GlobalShortcutsSession {
        state: state.clone(),
        app: req.app_id.to_string(),
        session_obj: obj,
        shortcuts: Default::default(),
    });
    {
        use org::freedesktop::impl_::portal::session::*;
        let ses = Rc::downgrade(&session);
        session.session_obj.add_method::<Close, _>(move |_, pr| {
            if let Some(ses) = ses.upgrade() {
                ses.kill();
            }
            pr.ok(&SessionCloseReply);
        });
        session.session_obj.set_property::<version>(Variant::U32(2));
    }
    state
        .global_shortcuts_sessions
        .set(req.session_handle.0.to_string(), session);
    reply.ok(&CreateSessionReply {
        response: PORTAL_SUCCESS,
        results: Default::default(),
    });
}

fn has_conflicting_shortcut(
    state: &PortalState,
    session: &Rc<GlobalShortcutsSession>,
    shortcut_id: &str,
) -> bool {
    state
        .global_shortcuts_sessions
        .lock()
        .values()
        .any(|other| {
            !Rc::ptr_eq(other, session)
                && other.app == session.app
                && other
                    .shortcuts
                    .borrow()
                    .iter()
                    .any(|s| s.shortcut_id == shortcut_id)
        })
}

fn dbus_bind_shortcuts(
    state: &Rc<PortalState>,
    req: BindShortcuts<'_>,
    reply: PendingReply<BindShortcutsReply<'static>>,
) {
    let Some(session) = get_session(state, &reply, &req.session_handle.0) else {
        return;
    };
    for (shortcut_id, _) in req.shortcuts.iter() {
        if has_conflicting_shortcut(state, &session, shortcut_id.deref()) {
            let msg = format!(
                "Global shortcut `{}:{}` is already registered by another session",
                session.app,
                shortcut_id.deref(),
            );
            log::warn!("{msg}");
            reply.err(&msg);
            return;
        }
    }
    log::info!(
        "Bind {} global shortcut(s) for `{}` ({}):",
        req.shortcuts.len(),
        session.app,
        session.session_obj.path(),
    );
    let session_weak = Rc::downgrade(&session);
    for (shortcut_id, vardict) in req.shortcuts.iter() {
        let mut description = "";
        let mut preferred_trigger = "";
        for entry in vardict.iter() {
            match entry.key.deref() {
                "description" => {
                    if let Variant::String(s) = &entry.value {
                        description = s.deref();
                    }
                }
                "preferred_trigger" => {
                    if let Variant::String(s) = &entry.value {
                        preferred_trigger = s.deref();
                    }
                }
                _ => {}
            }
        }
        let trigger_description = if preferred_trigger.is_empty() {
            "configured in jay".to_string()
        } else {
            preferred_trigger.to_string()
        };
        log::info!(
            "  global-shortcut:{}:{} ({})",
            session.app,
            shortcut_id.deref(),
            description,
        );
        let old = {
            let mut shortcuts = session.shortcuts.borrow_mut();
            shortcuts
                .iter()
                .position(|s| s.shortcut_id == shortcut_id.deref())
                .map(|pos| shortcuts.remove(pos))
        };
        if let Some(old) = old {
            old.destroy_all();
        }
        let bound = Rc::new(BoundShortcut {
            session: session_weak.clone(),
            shortcut_id: shortcut_id.to_string(),
            description: description.to_string(),
            preferred_trigger: preferred_trigger.to_string(),
            trigger_description,
            displays: Default::default(),
        });
        for dpy in state.displays.lock().values() {
            if let Some(gs) = dpy.global_shortcuts.get() {
                bound.register_on_display(&dpy, &gs, &session.app);
            }
        }
        session.shortcuts.borrow_mut().push(bound);
    }
    let shortcuts = session.shortcuts.borrow();
    let results = build_shortcuts_results(&shortcuts);
    reply.ok(&BindShortcutsReply {
        response: PORTAL_SUCCESS,
        results: Cow::Borrowed(&results),
    });
}

fn dbus_list_shortcuts(
    state: &Rc<PortalState>,
    req: ListShortcuts<'_>,
    reply: PendingReply<ListShortcutsReply<'static>>,
) {
    let Some(session) = get_session(state, &reply, &req.session_handle.0) else {
        return;
    };
    let shortcuts = session.shortcuts.borrow();
    let results = build_shortcuts_results(&shortcuts);
    reply.ok(&ListShortcutsReply {
        response: PORTAL_SUCCESS,
        results: Cow::Borrowed(&results),
    });
}

fn dbus_configure_shortcuts(
    state: &Rc<PortalState>,
    req: ConfigureShortcuts<'_>,
    reply: PendingReply<ConfigureShortcutsReply>,
) {
    if get_session(state, &reply, &req.session_handle.0).is_some() {
        reply.ok(&ConfigureShortcutsReply);
    }
}

fn build_shortcuts_results(
    shortcuts: &[Rc<BoundShortcut>],
) -> Vec<DictEntry<Cow<'static, str>, Variant<'static>>> {
    let dict_entry_type = DynamicType::DictEntry(
        Box::new(DynamicType::String),
        Box::new(DynamicType::Variant),
    );
    let shortcut_type = DynamicType::Struct(vec![
        DynamicType::String,
        DynamicType::Array(Box::new(dict_entry_type.clone())),
    ]);
    let entries = shortcuts
        .iter()
        .map(|s| {
            Variant::Struct(vec![
                Variant::String(s.shortcut_id.clone().into()),
                Variant::Array(
                    dict_entry_type.clone(),
                    vec![
                        Variant::DictEntry(
                            Box::new(Variant::String("description".into())),
                            Box::new(Variant::Variant(Box::new(Variant::String(
                                s.description.clone().into(),
                            )))),
                        ),
                        Variant::DictEntry(
                            Box::new(Variant::String("trigger_description".into())),
                            Box::new(Variant::Variant(Box::new(Variant::String(
                                s.trigger_description.clone().into(),
                            )))),
                        ),
                    ],
                ),
            ])
        })
        .collect();
    vec![DictEntry {
        key: "shortcuts".into(),
        value: Variant::Array(shortcut_type, entries),
    }]
}

fn get_session<T>(
    state: &Rc<PortalState>,
    reply: &PendingReply<T>,
    handle: &str,
) -> Option<Rc<GlobalShortcutsSession>> {
    let res = state.global_shortcuts_sessions.get(handle);
    if res.is_none() {
        let msg = format!("Global shortcuts session `{handle}` does not exist");
        reply.err(&msg);
    }
    res
}
