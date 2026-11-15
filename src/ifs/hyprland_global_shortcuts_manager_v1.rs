use crate::client::Client;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::hyprland_global_shortcut_v1::HyprlandGlobalShortcutV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::utils::bhash::BHashMap;
use crate::wire::HyprlandGlobalShortcutV1Id;
use crate::wire::HyprlandGlobalShortcutsManagerV1Id;
use crate::wire::hyprland_global_shortcuts_manager_v1::*;
use hashbrown::Equivalent;
use jay_config::keyboard::mods::ALT;
use jay_config::keyboard::mods::CTRL;
use jay_config::keyboard::mods::LOCK;
use jay_config::keyboard::mods::LOGO;
use jay_config::keyboard::mods::MOD3;
use jay_config::keyboard::mods::MOD5;
use jay_config::keyboard::mods::Modifiers;
use jay_config::keyboard::mods::NUM;
use jay_config::keyboard::mods::RELEASE;
use jay_config::keyboard::mods::SHIFT;
use jay_config::keyboard::syms::KeySym;
use jay_proc::Global;
use jay_proc::Object;
use kbvm::Keysym;
use kbvm::lookup::Lookup;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::ptr;
use std::rc::Rc;
use thiserror::Error;

#[derive(Hash)]
struct ShortcutKeyRef<'a>(&'a str, &'a str);

impl Equivalent<(String, String)> for ShortcutKeyRef<'_> {
    fn equivalent(&self, key: &(String, String)) -> bool {
        self.0 == key.0 && self.1 == key.1
    }
}

struct ConfiguredTrigger {
    mods: Modifiers,
    sym: KeySym,
    description: String,
}

pub struct RegisteredGlobalShortcutInfo {
    pub app_id: String,
    pub shortcut_id: String,
    pub description: String,
    pub preferred_trigger: String,
    pub configured_trigger: String,
}

pub struct ConfiguredGlobalTriggerInfo {
    pub app_id: String,
    pub shortcut_id: String,
    pub trigger: String,
}

#[derive(Default)]
pub struct GlobalShortcutsRegistry {
    shortcuts: RefCell<BHashMap<(String, String), Rc<HyprlandGlobalShortcutV1>>>,
    configured_triggers: RefCell<BHashMap<(String, String), ConfiguredTrigger>>,
}

impl GlobalShortcutsRegistry {
    pub fn set_configured_trigger(
        &self,
        app_id: &str,
        shortcut_id: &str,
        mods: Modifiers,
        sym: KeySym,
        trigger_description: &str,
    ) {
        self.configured_triggers.borrow_mut().insert(
            (app_id.to_string(), shortcut_id.to_string()),
            ConfiguredTrigger {
                mods: Modifiers(mods.0 & !RELEASE.0),
                sym,
                description: trigger_description.to_string(),
            },
        );
    }

    pub fn clear_configured_triggers(&self) {
        self.configured_triggers.borrow_mut().clear();
    }

    pub fn clear(&self) {
        let shortcuts = mem::take(&mut *self.shortcuts.borrow_mut());
        let configured = mem::take(&mut *self.configured_triggers.borrow_mut());
        drop(shortcuts);
        drop(configured);
    }

    pub fn snapshot(
        &self,
    ) -> (
        Vec<RegisteredGlobalShortcutInfo>,
        Vec<ConfiguredGlobalTriggerInfo>,
    ) {
        let shortcuts = self.shortcuts.borrow();
        let configured = self.configured_triggers.borrow();
        let mut registered: Vec<_> = shortcuts
            .values()
            .map(|sc| {
                let configured_trigger = configured
                    .get(&ShortcutKeyRef(&sc.app_id, &sc.shortcut_id))
                    .map(|t| t.description.clone())
                    .unwrap_or_default();
                RegisteredGlobalShortcutInfo {
                    app_id: sc.app_id.clone(),
                    shortcut_id: sc.shortcut_id.clone(),
                    description: sc.description.clone(),
                    preferred_trigger: sc.preferred_trigger_description.clone(),
                    configured_trigger,
                }
            })
            .collect();
        registered.sort_by(|a, b| (&a.app_id, &a.shortcut_id).cmp(&(&b.app_id, &b.shortcut_id)));
        let mut triggers: Vec<_> = configured
            .iter()
            .map(
                |((app_id, shortcut_id), trigger)| ConfiguredGlobalTriggerInfo {
                    app_id: app_id.clone(),
                    shortcut_id: shortcut_id.clone(),
                    trigger: trigger.description.clone(),
                },
            )
            .collect();
        triggers.sort_by(|a, b| (&a.app_id, &a.shortcut_id).cmp(&(&b.app_id, &b.shortcut_id)));
        (registered, triggers)
    }

    #[expect(clippy::too_many_arguments)]
    pub fn register_shortcut(
        &self,
        client: &Rc<Client>,
        version: Version,
        id: HyprlandGlobalShortcutV1Id,
        shortcut_id: &str,
        app_id: &str,
        description: &str,
        trigger_description: &str,
    ) -> Result<(), HyprlandGlobalShortcutsManagerV1Error> {
        {
            let shortcuts = self.shortcuts.borrow();
            if shortcuts.contains_key(&ShortcutKeyRef(app_id, shortcut_id)) {
                return Err(HyprlandGlobalShortcutsManagerV1Error::AlreadyTaken);
            }
        }
        let preferred_trigger = parse_trigger_description(trigger_description);
        let shortcut = Rc::new(HyprlandGlobalShortcutV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            app_id: app_id.to_string(),
            shortcut_id: shortcut_id.to_string(),
            description: description.to_string(),
            preferred_trigger_description: trigger_description.to_string(),
            preferred_trigger,
            pressed_keycode: Cell::new(None),
        });
        track!(client, shortcut);
        client.add_client_obj(&shortcut);
        self.shortcuts
            .borrow_mut()
            .insert((app_id.to_string(), shortcut_id.to_string()), shortcut);
        Ok(())
    }

    pub fn remove_shortcut(&self, shortcut: &HyprlandGlobalShortcutV1) {
        let removed = {
            let mut shortcuts = self.shortcuts.borrow_mut();
            let key = ShortcutKeyRef(&shortcut.app_id, &shortcut.shortcut_id);
            if shortcuts
                .get(&key)
                .is_some_and(|existing| ptr::eq(Rc::as_ptr(existing), shortcut))
            {
                shortcuts.remove(&key)
            } else {
                None
            }
        };
        drop(removed);
    }

    pub fn handle_key_pressed(
        &self,
        time_usec: u64,
        keycode: u32,
        mods: Modifiers,
        keysyms: Lookup<'_>,
    ) {
        let shortcuts = self.shortcuts.borrow();
        if shortcuts.is_empty() {
            return;
        }
        let configured = self.configured_triggers.borrow();
        for shortcut in shortcuts.values() {
            if shortcut.pressed_keycode.get().is_some() {
                continue;
            }
            let effective = configured
                .get(&ShortcutKeyRef(&shortcut.app_id, &shortcut.shortcut_id))
                .map(|t| (t.mods, t.sym))
                .or(shortcut.preferred_trigger);
            let Some((trigger_mods, trigger_sym)) = effective else {
                continue;
            };
            if trigger_mods != mods {
                continue;
            }
            let trigger_lower = Keysym(trigger_sym.0).to_lowercase().0;
            let matched = keysyms.into_iter().any(|props| {
                let sym = props.keysym();
                sym.0 == trigger_sym.0 || sym.to_lowercase().0 == trigger_lower
            });
            if matched {
                shortcut.pressed_keycode.set(Some(keycode));
                shortcut.send_pressed(time_usec);
            }
        }
    }

    pub fn handle_key_released(&self, time_usec: u64, keycode: u32) {
        let shortcuts = self.shortcuts.borrow();
        if shortcuts.is_empty() {
            return;
        }
        for shortcut in shortcuts.values() {
            if shortcut.pressed_keycode.get() == Some(keycode) {
                shortcut.pressed_keycode.set(None);
                shortcut.send_released(time_usec);
            }
        }
    }
}

pub fn parse_trigger_description(desc: &str) -> Option<(Modifiers, KeySym)> {
    let desc = desc.trim();
    if desc.is_empty() {
        return None;
    }
    let mut mods = Modifiers::NONE;
    let mut sym = None;
    for part in desc.split(['+', '-']) {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        if let Some(m) = parse_modifier(part) {
            mods |= m;
            continue;
        }
        if sym.is_some() {
            return None;
        }
        let parsed = Keysym::from_str(part)
            .or_else(|| Keysym::from_str(&part.to_ascii_lowercase()))
            .or_else(|| Keysym::from_str_insensitive(part))?;
        sym = Some(KeySym(parsed.0));
    }
    sym.map(|sym| (mods, sym))
}

fn parse_modifier(part: &str) -> Option<Modifiers> {
    if part.eq_ignore_ascii_case("ctrl") || part.eq_ignore_ascii_case("control") {
        Some(CTRL)
    } else if part.eq_ignore_ascii_case("shift") {
        Some(SHIFT)
    } else if part.eq_ignore_ascii_case("alt") || part.eq_ignore_ascii_case("mod1") {
        Some(ALT)
    } else if part.eq_ignore_ascii_case("super")
        || part.eq_ignore_ascii_case("logo")
        || part.eq_ignore_ascii_case("meta")
        || part.eq_ignore_ascii_case("win")
        || part.eq_ignore_ascii_case("mod4")
    {
        Some(LOGO)
    } else if part.eq_ignore_ascii_case("caps") || part.eq_ignore_ascii_case("lock") {
        Some(LOCK)
    } else if part.eq_ignore_ascii_case("num") || part.eq_ignore_ascii_case("mod2") {
        Some(NUM)
    } else if part.eq_ignore_ascii_case("mod3") {
        Some(MOD3)
    } else if part.eq_ignore_ascii_case("mod5") {
        Some(MOD5)
    } else {
        None
    }
}

#[derive(Global)]
pub struct HyprlandGlobalShortcutsManagerV1Global {
    name: GlobalName,
}

impl HyprlandGlobalShortcutsManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: HyprlandGlobalShortcutsManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), !> {
        let obj = Rc::new(HyprlandGlobalShortcutsManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for HyprlandGlobalShortcutsManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

#[derive(Object)]
pub struct HyprlandGlobalShortcutsManagerV1 {
    id: HyprlandGlobalShortcutsManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl HyprlandGlobalShortcutsManagerV1RequestHandler for HyprlandGlobalShortcutsManagerV1 {
    type Error = HyprlandGlobalShortcutsManagerV1Error;

    fn register_shortcut(
        &self,
        req: RegisterShortcut<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.client.state.global_shortcuts.register_shortcut(
            &self.client,
            self.version,
            req.shortcut,
            req.id,
            req.app_id,
            req.description,
            req.trigger_description,
        )
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum HyprlandGlobalShortcutsManagerV1Error {
    #[error("The global shortcut is already registered")]
    AlreadyTaken,
}
