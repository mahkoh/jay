use crate::backend::KeyState;
use crate::client::Client;
use crate::ifs::wl_seat::wl_keyboard;
use crate::it::test_client::TestClient;
use crate::it::test_ifs::test_seat::TestSeat;
use crate::wire::ZwpVirtualKeyboardV1Id;
use std::io::Write;
use std::rc::Rc;
use uapi::c;

pub struct TestVirtualKeyboard {
    pub id: ZwpVirtualKeyboardV1Id,
    pub client: Rc<Client>,
}

impl TestVirtualKeyboard {
    pub fn set_keymap(&self, map: &str) {
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
        self.client.send_zwp_virtual_keyboard_v1_keymap(
            self.id,
            wl_keyboard::XKB_V1,
            &Rc::new(memfd),
            map.len() as _,
        );
    }

    pub fn key(&self, key: u32, state: KeyState) {
        let state = match state {
            KeyState::Released => wl_keyboard::RELEASED,
            KeyState::Pressed => wl_keyboard::PRESSED,
            KeyState::Repeated => wl_keyboard::REPEATED,
        };
        self.client.send_zwp_virtual_keyboard_v1_key(
            self.id,
            self.client.state.now_msec() as u32,
            key,
            state,
        );
    }

    pub fn modifiers(&self, mods_depressed: u32, mods_latched: u32, mods_locked: u32, group: u32) {
        self.client.send_zwp_virtual_keyboard_v1_modifiers(
            self.id,
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        );
    }
}

impl TestClient {
    pub fn create_virtual_keyboard(&self, seat: &TestSeat) -> Rc<TestVirtualKeyboard> {
        let id = self
            .client
            .send_zwp_virtual_keyboard_manager_v1_create_virtual_keyboard(seat.id);
        Rc::new(TestVirtualKeyboard {
            id,
            client: self.client.clone(),
        })
    }
}
