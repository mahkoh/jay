use {
    crate::utils::vecset::VecSet,
    kbvm::Components,
    std::{
        cell::{Ref, RefCell},
        rc::Rc,
    },
    uapi::OwnedFd,
};

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
