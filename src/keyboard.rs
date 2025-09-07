use {
    crate::utils::{event_listener::EventSource, oserror::OsError, vecset::VecSet},
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
    pub map: KeymapFd,
    pub xwayland_map: KeymapFd,
    pub pressed_keys: VecSet<u32>,
    pub mods: Components,
    pub mods_changed: EventSource<dyn ModifiersListener>,
}

pub trait ModifiersListener {
    fn locked_mods(&self, mods: &Components);
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
        let locked_mods = self.mods.mods_locked;
        let changed = self.mods.apply_event(event);
        if locked_mods != self.mods.mods_locked {
            self.dispatch_locked_mods_listeners();
        }
        changed
    }

    pub fn dispatch_locked_mods_listeners(&self) {
        for listener in self.mods_changed.iter() {
            listener.locked_mods(&self.mods);
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
