use crate::it::test_error::TestErrorError;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::wire::zwp_input_method_keyboard_grab_v2::*;
use std::rc::Rc;

#[derive(Default)]
pub struct TestInputMethodKeyboardGrab {
    pub keymap: TEEH<Keymap>,
    pub key: TEEH<Key>,
    pub modifiers: TEEH<Modifiers>,
    pub repeat_info: TEEH<RepeatInfo>,
}

synthetic_event_handler!(TestInputMethodKeyboardGrab);

impl ZwpInputMethodKeyboardGrabV2EventHandler for TestInputMethodKeyboardGrab {
    type Error = TestErrorError;

    fn keymap(&self, ev: Keymap, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.keymap.push(ev);
        Ok(())
    }

    fn key(&self, ev: Key, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.key.push(ev);
        Ok(())
    }

    fn modifiers(&self, ev: Modifiers, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.modifiers.push(ev);
        Ok(())
    }

    fn repeat_info(&self, ev: RepeatInfo, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.repeat_info.push(ev);
        Ok(())
    }
}
