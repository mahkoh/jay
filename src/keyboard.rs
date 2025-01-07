use {
    crate::utils::vecset::VecSet,
    std::{
        cell::{Ref, RefCell},
        rc::Rc,
    },
    uapi::OwnedFd,
};

#[derive(Copy, Clone, Debug, Default)]
pub struct ModifierState {
    pub mods_depressed: u32,
    pub mods_latched: u32,
    pub mods_locked: u32,
    pub mods_effective: u32,
    pub group: u32,
}

linear_ids!(KeyboardStateIds, KeyboardStateId, u64);

pub struct KeyboardState {
    pub id: KeyboardStateId,
    pub map: Rc<OwnedFd>,
    pub map_len: usize,
    pub pressed_keys: VecSet<u32>,
    pub mods: ModifierState,
}

pub trait DynKeyboardState {
    fn borrow(&self) -> Ref<'_, KeyboardState>;
}

impl DynKeyboardState for RefCell<KeyboardState> {
    fn borrow(&self) -> Ref<'_, KeyboardState> {
        self.borrow()
    }
}
