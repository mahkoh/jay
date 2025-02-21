use {
    crate::{
        backend::KeyState,
        ifs::wl_seat::wl_keyboard,
        it::{test_error::TestError, test_object::TestObject, test_transport::TestTransport},
        wire::{ZwpVirtualKeyboardV1Id, zwp_virtual_keyboard_v1::*},
    },
    std::{cell::Cell, io::Write, rc::Rc},
    uapi::c,
};

pub struct TestVirtualKeyboard {
    pub id: ZwpVirtualKeyboardV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestVirtualKeyboard {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn set_keymap(&self, map: &str) -> Result<(), TestError> {
        let mut memfd =
            uapi::memfd_create("keymap", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING).unwrap();
        memfd.write_all(map.as_bytes()).unwrap();
        memfd.write_all(&[0]).unwrap();
        uapi::lseek(memfd.raw(), 0, c::SEEK_SET).unwrap();
        uapi::fcntl_add_seals(
            memfd.raw(),
            c::F_SEAL_SEAL | c::F_SEAL_GROW | c::F_SEAL_SHRINK,
        )
        .unwrap();
        self.tran.send(Keymap {
            self_id: self.id,
            format: wl_keyboard::XKB_V1,
            fd: Rc::new(memfd),
            size: map.len() as _,
        })
    }

    pub fn key(&self, key: u32, state: KeyState) -> Result<(), TestError> {
        let state = match state {
            KeyState::Released => wl_keyboard::RELEASED,
            KeyState::Pressed => wl_keyboard::PRESSED,
        };
        self.tran.send(Key {
            self_id: self.id,
            time: self.tran.run.state.now_msec() as u32,
            key,
            state,
        })
    }

    pub fn modifiers(
        &self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) -> Result<(), TestError> {
        self.tran.send(Modifiers {
            self_id: self.id,
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        })
    }
}

impl Drop for TestVirtualKeyboard {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestVirtualKeyboard, ZwpVirtualKeyboardV1;
}

impl TestObject for TestVirtualKeyboard {}
