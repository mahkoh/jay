use {
    crate::{
        backend::{LED_CAPS_LOCK, LED_COMPOSE, LED_KANA, LED_NUM_LOCK, LED_SCROLL_LOCK, Leds},
        kbvm::KbvmMap,
        utils::{event_listener::EventSource, oserror::OsError, vecset::VecSet},
    },
    kbvm::{Components, state_machine::Event},
    std::{
        cell::{Ref, RefCell},
        rc::Rc,
    },
    thiserror::Error,
    uapi::{Errno, OwnedFd, c},
};

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
        let fd = match uapi::memfd_create("shared-keymap", c::MFD_CLOEXEC) {
            Ok(fd) => fd,
            Err(e) => return Err(KeyboardError::KeymapMemfd(e.into())),
        };
        let target = self.len as c::off_t;
        let mut pos = 0;
        while pos < target {
            let rem = target - pos;
            let res = uapi::sendfile(fd.raw(), self.map.raw(), Some(&mut pos), rem as usize);
            match res {
                Ok(_) | Err(Errno(c::EINTR)) => {}
                Err(e) => return Err(KeyboardError::KeymapCopy(e.into())),
            }
        }
        Ok(Self {
            map: Rc::new(fd),
            len: self.len,
        })
    }
}
