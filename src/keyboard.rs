use {
    crate::utils::{oserror::OsError, vecset::VecSet},
    kbvm::Components,
    std::{
        cell::{Ref, RefCell},
        rc::Rc,
    },
    thiserror::Error,
    uapi::{c, Errno, OwnedFd},
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
    pub map: Rc<OwnedFd>,
    pub map_len: usize,
    pub pressed_keys: VecSet<u32>,
    pub mods: Components,
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
    pub fn create_new_keymap_fd(&self) -> Result<Rc<OwnedFd>, KeyboardError> {
        let fd = match uapi::memfd_create("shared-keymap", c::MFD_CLOEXEC) {
            Ok(fd) => fd,
            Err(e) => return Err(KeyboardError::KeymapMemfd(e.into())),
        };
        let target = self.map_len as c::off_t;
        let mut pos = 0;
        while pos < target {
            let rem = target - pos;
            let res = uapi::sendfile(fd.raw(), self.map.raw(), Some(&mut pos), rem as usize);
            match res {
                Ok(_) | Err(Errno(c::EINTR)) => {}
                Err(e) => return Err(KeyboardError::KeymapCopy(e.into())),
            }
        }
        Ok(Rc::new(fd))
    }
}
