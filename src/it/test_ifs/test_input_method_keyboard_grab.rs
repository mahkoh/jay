use {
    crate::{
        it::{
            test_error::TestError, test_object::TestObject, test_transport::TestTransport,
            test_utils::test_expected_event::TEEH, testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{ZwpInputMethodKeyboardGrabV2Id, zwp_input_method_keyboard_grab_v2::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestInputMethodKeyboardGrab {
    pub id: ZwpInputMethodKeyboardGrabV2Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub keymap: TEEH<Keymap>,
    pub key: TEEH<Key>,
    pub modifiers: TEEH<Modifiers>,
    pub repeat_info: TEEH<RepeatInfo>,
}

impl TestInputMethodKeyboardGrab {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Release { self_id: self.id })?;
        }
        Ok(())
    }

    fn handle_keymap(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Keymap::parse_full(parser)?;
        self.keymap.push(ev);
        Ok(())
    }

    fn handle_key(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Key::parse_full(parser)?;
        self.key.push(ev);
        Ok(())
    }

    fn handle_modifiers(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Modifiers::parse_full(parser)?;
        self.modifiers.push(ev);
        Ok(())
    }

    fn handle_repeat_info(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = RepeatInfo::parse_full(parser)?;
        self.repeat_info.push(ev);
        Ok(())
    }
}

impl Drop for TestInputMethodKeyboardGrab {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestInputMethodKeyboardGrab, ZwpInputMethodKeyboardGrabV2;

    KEYMAP => handle_keymap,
    KEY => handle_key,
    MODIFIERS => handle_modifiers,
    REPEAT_INFO => handle_repeat_info,
}

impl TestObject for TestInputMethodKeyboardGrab {}
