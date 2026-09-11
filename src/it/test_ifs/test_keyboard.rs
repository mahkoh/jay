use crate::it::test_error::TestErrorError;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::utils::numcell::NumCell;
use crate::wire::WlSurfaceId;
use crate::wire::wl_keyboard::*;
use std::rc::Rc;

pub struct TestEnterEvent {
    pub serial: u32,
    pub surface: WlSurfaceId,
    _keys: Vec<u32>,
}

pub struct TestKeyboard {
    pub keymap: TEEH<(usize, Keymap)>,
    pub key: TEEH<(usize, Key)>,
    pub modifiers: TEEH<(usize, Modifiers)>,
    pub enter: TEEH<TestEnterEvent>,
    pub leave: TEEH<Leave>,
    pub event_id: NumCell<usize>,
}

synthetic_event_handler!(TestKeyboard);

impl WlKeyboardEventHandler for TestKeyboard {
    type Error = TestErrorError;

    fn keymap(&self, ev: Keymap, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.keymap.push((self.event_id.fetch_add(1), ev));
        Ok(())
    }

    fn enter(&self, ev: Enter<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.enter.push(TestEnterEvent {
            serial: ev.serial,
            surface: ev.surface,
            _keys: ev.keys.to_vec(),
        });
        Ok(())
    }

    fn leave(&self, ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.leave.push(ev);
        Ok(())
    }

    fn key(&self, ev: Key, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.key.push((self.event_id.fetch_add(1), ev));
        Ok(())
    }

    fn modifiers(&self, ev: Modifiers, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.modifiers.push((self.event_id.fetch_add(1), ev));
        Ok(())
    }

    fn repeat_info(&self, _ev: RepeatInfo, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}
