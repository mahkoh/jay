use crate::backend::LED_CAPS_LOCK;
use crate::backend::LED_COMPOSE;
use crate::backend::LED_KANA;
use crate::backend::LED_NUM_LOCK;
use crate::backend::LED_SCROLL_LOCK;
use crate::backend::Leds;
use crate::kbvm::KbvmMap;
use crate::utils::event_listener::EventSource;
use crate::utils::oserror::OsError;
use crate::utils::oserror::OsErrorExt;
use crate::utils::oserror::OsErrorExt2;
use crate::utils::vecset::VecSet;
use kbvm::Components;
use kbvm::state_machine::Event;
use std::cell::Ref;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;

#[derive(Debug, Error)]
pub enum KeyboardError {
    #[error("Could not create a keymap memfd")]
    KeymapMemfd(#[source] OsError),
    #[error("Could not copy the keymap")]
    KeymapCopy(#[source] OsError),
}

linear_ids!(KeyboardStateIds, KeyboardStateId, u64);

pub struct KeyboardState {
    pub id: KeyboardStateId,
    pub map: Rc<KbvmMap>,
    pub pressed_keys: VecSet<u32>,
    pub mods: Components,
    pub leds: Leds,
    pub leds_changed: EventSource<dyn LedsListener>,
}

pub trait LedsListener {
    fn leds(&self, leds: Leds);
}

pub trait DynKeyboardState {
    fn borrow(&self) -> Ref<'_, KeyboardState>;
}

impl DynKeyboardState for RefCell<KeyboardState> {
    fn borrow(&self) -> Ref<'_, KeyboardState> {
        self.borrow()
    }
}

impl KeyboardState {
    pub fn apply_event(&mut self, event: Event) -> bool {
        let changed = self.mods.apply_event(event);
        if changed && self.map.has_indicators {
            self.update_leds();
        }
        changed
    }

    pub fn update_leds(&mut self) {
        if !self.map.has_indicators {
            return;
        }
        let mut new = Leds::none();
        macro_rules! map_led {
            ($field:ident, $led:ident) => {
                if let Some(m) = &self.map.$field
                    && m.matches(&self.mods)
                {
                    new |= $led;
                }
            };
        }
        map_led!(num_lock, LED_NUM_LOCK);
        map_led!(caps_lock, LED_CAPS_LOCK);
        map_led!(scroll_lock, LED_SCROLL_LOCK);
        map_led!(compose, LED_COMPOSE);
        map_led!(kana, LED_KANA);
        if new != self.leds {
            self.leds = new;
            for listener in self.leds_changed.iter() {
                listener.leds(new);
            }
        }
    }
}

#[derive(Clone)]
pub struct KeymapFd {
    pub map: Rc<OwnedFd>,
    pub len: usize,
}

impl KeymapFd {
    pub fn create_unprotected_fd(&self) -> Result<Self, KeyboardError> {
        let fd = uapi::memfd_create("shared-keymap", c::MFD_CLOEXEC)
            .map_os_err(KeyboardError::KeymapMemfd)?;
        let target = self.len as c::off_t;
        let mut pos = 0;
        while pos < target {
            let rem = target - pos;
            let res = uapi::sendfile(fd.raw(), self.map.raw(), Some(&mut pos), rem as usize)
                .to_os_error();
            match res {
                Ok(_) | Err(OsError(c::EINTR)) => {}
                Err(e) => return Err(KeyboardError::KeymapCopy(e)),
            }
        }
        Ok(Self {
            map: Rc::new(fd),
            len: self.len,
        })
    }
}
