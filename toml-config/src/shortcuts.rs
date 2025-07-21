use {
    crate::{
        State,
        config::{Action, InputMode, Shortcut, SimpleCommand},
    },
    ahash::{AHashMap, AHashSet},
    jay_config::keyboard::{ModifiedKeySym, mods::Modifiers},
    std::{
        cell::{Cell, RefCell},
        collections::hash_map::Entry,
        rc::Rc,
    },
};

#[derive(Default)]
pub struct ModeState {
    latched: Cell<bool>,
    stack: RefCell<Vec<Rc<ConvertedShortcuts>>>,
    slots: RefCell<AHashMap<String, Rc<ModeSlot>>>,
    diffs: RefCell<AHashMap<[*const ConvertedShortcuts; 2], Rc<Vec<ModeDiff>>>>,
    current: RefCell<Rc<ConvertedShortcuts>>,
}

impl ModeState {
    pub fn clear(&self) {
        self.slots.borrow_mut().clear();
        self.stack.borrow_mut().clear();
        self.diffs.borrow_mut().clear();
        *self.current.borrow_mut() = Default::default();
    }
}

pub type ConvertedShortcuts = AHashMap<ModifiedKeySym, ConvertedShortcut>;

#[derive(Clone)]
pub struct ConvertedShortcut {
    mask: Modifiers,
    shortcut: Rc<dyn Fn()>,
}

#[derive(Default)]
pub struct ModeSlot {
    pub mode: RefCell<Option<Rc<ConvertedShortcuts>>>,
}

enum ModeDiff {
    Bind(ModifiedKeySym, Modifiers, Rc<dyn Fn()>),
    Unbind(ModifiedKeySym),
}

impl PartialEq for ConvertedShortcut {
    fn eq(&self, other: &Self) -> bool {
        if self.mask != other.mask {
            return false;
        }
        Rc::ptr_eq(&self.shortcut, &other.shortcut)
    }
}

impl State {
    pub fn get_mode_slot(&self, name: &str) -> Rc<ModeSlot> {
        let state = &self.persistent.mode_state;
        state
            .slots
            .borrow_mut()
            .entry(name.to_string())
            .or_default()
            .clone()
    }

    pub fn clear_modes_after_reload(&self) {
        let state = &self.persistent.mode_state;
        state.slots.borrow_mut().clear();
        state.diffs.borrow_mut().clear();
    }

    pub fn init_modes(
        self: &Rc<Self>,
        shortcuts: &[Shortcut],
        modes: &AHashMap<String, InputMode>,
    ) {
        let state = &self.persistent.mode_state;
        let base = self.convert_shortcuts(shortcuts);
        let stack = &mut *state.stack.borrow_mut();
        stack.clear();
        stack.push(base.clone());
        self.convert_modes(&base, modes);
        self.apply_shortcuts(&base);
        state.latched.set(false);
    }

    pub fn set_mode(&self, new: &Rc<ConvertedShortcuts>, latch: bool) {
        let state = &self.persistent.mode_state;
        self.cancel_mode_latch();
        self.apply_shortcuts(new);
        let stack = &mut *state.stack.borrow_mut();
        stack.push(new.clone());
        if latch {
            state.latched.set(true);
        }
    }

    pub fn pop_mode(&self, pop: bool) {
        let state = &self.persistent.mode_state;
        let stack = &mut *state.stack.borrow_mut();
        if stack.len() < 1 + pop as usize {
            log::error!("Mode stack is empty");
            return;
        }
        self.cancel_mode_latch();
        if pop {
            stack.pop();
        } else {
            stack.truncate(1);
        }
        let new = stack.last().unwrap();
        self.apply_shortcuts(new);
    }

    pub fn cancel_mode_latch(&self) {
        let state = &self.persistent.mode_state;
        if !state.latched.take() {
            return;
        }
        let stack = &mut *state.stack.borrow_mut();
        if stack.len() < 2 {
            log::error!("Mode is latched but mode stack is empty");
            return;
        }
        let _ = stack.pop();
        let new = stack.last().unwrap();
        self.apply_shortcuts(new);
    }

    pub fn convert_modes(
        self: &Rc<Self>,
        base: &ConvertedShortcuts,
        modes: &AHashMap<String, InputMode>,
    ) {
        let mut pending = AHashSet::new();
        let mut out = AHashMap::new();
        for (name, mode) in modes {
            if !out.contains_key(name) {
                self.convert_mode(&mut out, &mut pending, base, modes, name, mode);
            }
        }
    }

    fn convert_mode<'a>(
        self: &Rc<Self>,
        out: &'a mut AHashMap<String, Rc<ConvertedShortcuts>>,
        pending: &mut AHashSet<String>,
        base: &ConvertedShortcuts,
        modes: &AHashMap<String, InputMode>,
        mode_name: &String,
        mode: &InputMode,
    ) -> Option<&'a ConvertedShortcuts> {
        if !pending.insert(mode_name.clone()) {
            log::warn!("Detected loop while converting input mode `{mode_name}`");
            return None;
        }
        let mut shortcuts = None;
        if let Some(parent) = &mode.parent {
            match out.get(parent) {
                Some(c) => shortcuts = Some((**c).clone()),
                None => match modes.get(parent) {
                    None => {
                        log::warn!("Input mode `{parent}` does not exist");
                    }
                    Some(p) => {
                        if let Some(p) = self.convert_mode(out, pending, base, modes, parent, p) {
                            shortcuts = Some(p.clone());
                        }
                    }
                },
            }
        }
        let mut shortcuts = shortcuts.unwrap_or_else(|| base.clone());
        self.convert_shortcuts_(&mode.shortcuts, &mut shortcuts);
        let shortcuts = Rc::new(shortcuts);
        *self.get_mode_slot(mode_name).mode.borrow_mut() = Some(shortcuts.clone());
        let res = out.entry(mode_name.clone()).insert_entry(shortcuts);
        Some(res.into_mut())
    }

    pub fn convert_shortcuts<'a>(
        self: &Rc<Self>,
        shortcuts: impl IntoIterator<Item = &'a Shortcut>,
    ) -> Rc<ConvertedShortcuts> {
        let mut dst = ConvertedShortcuts::new();
        self.convert_shortcuts_(shortcuts, &mut dst);
        Rc::new(dst)
    }

    fn convert_shortcuts_<'a>(
        self: &Rc<Self>,
        shortcuts: impl IntoIterator<Item = &'a Shortcut>,
        dst: &mut ConvertedShortcuts,
    ) {
        for sc in shortcuts {
            match self.convert_shortcut(sc.clone()) {
                None => dst.remove(&sc.keysym),
                Some(cs) => dst.insert(sc.keysym, cs),
            };
        }
    }

    fn convert_shortcut(self: &Rc<Self>, shortcut: Shortcut) -> Option<ConvertedShortcut> {
        if let Action::SimpleCommand {
            cmd: SimpleCommand::None,
        } = shortcut.action
            && shortcut.latch.is_none()
        {
            return None;
        }
        let mut f = shortcut.action.into_shortcut_fn(self);
        if let Some(l) = shortcut.latch {
            let l = l.into_rc_fn(self);
            let s = self.persistent.seat;
            f = Rc::new(move || {
                f();
                let l = l.clone();
                s.latch(move || l());
            });
        }
        Some(ConvertedShortcut {
            mask: shortcut.mask,
            shortcut: f,
        })
    }

    pub fn apply_shortcuts(&self, new: &Rc<ConvertedShortcuts>) {
        let state = &self.persistent.mode_state;
        let current = &mut *state.current.borrow_mut();
        let diffs = self.get_or_create_mode_diffs(current, new);
        let seat = &self.persistent.seat;
        for diff in &*diffs {
            match diff {
                ModeDiff::Bind(key, mask, f) => {
                    let f = f.clone();
                    seat.bind_masked(*mask, *key, move || f());
                }
                ModeDiff::Unbind(key) => {
                    seat.unbind(*key);
                }
            }
        }
        *current = new.clone();
    }

    fn get_or_create_mode_diffs(
        &self,
        old: &Rc<ConvertedShortcuts>,
        new: &Rc<ConvertedShortcuts>,
    ) -> Rc<Vec<ModeDiff>> {
        let state = &self.persistent.mode_state;
        let diffs = &mut *state.diffs.borrow_mut();
        match diffs.entry([Rc::as_ptr(old), Rc::as_ptr(new)]) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => {
                let mut diffs = vec![];
                for (key, sc) in new.iter() {
                    if old.get(key) != Some(sc) {
                        diffs.push(ModeDiff::Bind(*key, sc.mask, sc.shortcut.clone()));
                    }
                }
                for key in old.keys() {
                    if !new.contains_key(key) {
                        diffs.push(ModeDiff::Unbind(*key));
                    }
                }
                v.insert(Rc::new(diffs)).clone()
            }
        }
    }
}
