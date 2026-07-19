use crate::ifs::wl_seat::wl_keyboard;
use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::utils::mmap::mmap;
use crate::utils::oserror::OsError;
use crate::wire::WlKeyboardId;
use crate::wire::WlSurfaceId;
use crate::wire::wl_keyboard::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use kbvm::Components;
use kbvm::Keycode;
use kbvm::ModifierMask;
use kbvm::lookup::Lookup;
use kbvm::lookup::LookupTable;
use kbvm::xkb::Context;
use kbvm::xkb::diagnostic::Diagnostic;
use kbvm::xkb::diagnostic::WriteToLog;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;
use uapi::c;

pub struct UsrWlKeyboard {
    pub id: WlKeyboardId,
    pub con: Rc<UsrCon>,
    pub keyboard: RefCell<Option<Keyboard>>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlKeyboardOwner>>>,
    pub version: Version,
}

pub struct Keyboard {
    lookup: LookupTable,
    components: Components,
}

pub trait UsrWlKeyboardOwner {
    fn focus(self: Rc<Self>, surface: WlSurfaceId, serial: u32);
    fn unfocus(self: Rc<Self>);
    fn modifiers(self: Rc<Self>, mods: ModifierMask);
    fn down(self: Rc<Self>, lookup: Lookup<'_>, serial: u32);
    fn repeat(self: Rc<Self>, lookup: Lookup<'_>, serial: u32);
    fn up(self: Rc<Self>, lookup: Lookup<'_>, serial: u32);
}

impl WlKeyboardEventHandler for UsrWlKeyboard {
    type Error = UsrWlKeyboardError;

    fn keymap(&self, ev: Keymap, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let map = mmap(ev.size as _, c::PROT_READ, c::MAP_PRIVATE, ev.fd.raw(), 0)
            .map_err(UsrWlKeyboardError::MapKeymap)?;
        let mut builder = Context::builder();
        builder.enable_default_includes(false);
        builder.enable_environment(false);
        let keymap = builder
            .build()
            .keymap_from_bytes(WriteToLog, None, unsafe { &*map.ptr })
            .map_err(UsrWlKeyboardError::ParseKeymap)?;
        let lookup = keymap.to_builder().build_lookup_table();
        let keyboard = Keyboard {
            lookup,
            components: Default::default(),
        };
        self.keyboard.replace(Some(keyboard));
        Ok(())
    }

    fn enter(&self, ev: Enter<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(owner) = self.owner.get() else {
            return Ok(());
        };
        owner.focus(ev.surface, ev.serial);
        Ok(())
    }

    fn leave(&self, _ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(owner) = self.owner.get() else {
            return Ok(());
        };
        owner.unfocus();
        Ok(())
    }

    fn key(&self, ev: Key, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(kb) = &*self.keyboard.borrow() else {
            return Ok(());
        };
        let Some(owner) = self.owner.get() else {
            return Ok(());
        };
        let kc = Keycode::from_evdev(ev.key);
        let lookup = kb
            .lookup
            .lookup(kb.components.group, kb.components.mods, kc);
        if ev.state == wl_keyboard::PRESSED {
            owner.down(lookup, ev.serial);
        } else if ev.state == wl_keyboard::REPEATED {
            owner.repeat(lookup, ev.serial);
        } else if ev.state == wl_keyboard::RELEASED {
            owner.up(lookup, ev.serial);
        }
        Ok(())
    }

    fn modifiers(&self, ev: Modifiers, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(kb) = &mut *self.keyboard.borrow_mut() else {
            return Ok(());
        };
        kb.components.mods_pressed.0 = ev.mods_depressed;
        kb.components.mods_latched.0 = ev.mods_latched;
        kb.components.mods_locked.0 = ev.mods_locked;
        kb.components.group_locked.0 = ev.group;
        let old = kb.components.mods;
        kb.components.update_effective();
        let new = kb.components.mods;
        if old != new
            && let Some(owner) = self.owner.get()
        {
            owner.modifiers(new);
        }
        Ok(())
    }

    fn repeat_info(&self, _ev: RepeatInfo, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlKeyboard = WlKeyboard;
    version = self.version;
}

impl UsrObject for UsrWlKeyboard {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}

#[derive(Debug, Error)]
pub enum UsrWlKeyboardError {
    #[error("Could not map the keymap")]
    MapKeymap(#[source] OsError),
    #[error("Could not parse the keymap")]
    ParseKeymap(#[source] Diagnostic),
}
