use {
    crate::{
        ifs::wl_seat::wl_keyboard::WlKeyboard,
        it::{
            test_error::TestResult, test_object::TestObject, test_transport::TestTransport,
            test_utils::test_expected_event::TEEH, testrun::ParseFull,
        },
        utils::{buffd::MsgParser, clonecell::CloneCell, numcell::NumCell, once::Once},
        wire::{wl_keyboard::*, WlKeyboardId, WlSurfaceId},
    },
    std::rc::Rc,
};

pub struct TestEnterEvent {
    pub serial: u32,
    pub surface: WlSurfaceId,
    pub keys: Vec<u32>,
}

pub struct TestKeyboard {
    pub id: WlKeyboardId,
    pub tran: Rc<TestTransport>,
    pub server: CloneCell<Option<Rc<WlKeyboard>>>,
    pub destroyed: Once,
    pub keymap: TEEH<(usize, Keymap)>,
    pub key: TEEH<(usize, Key)>,
    pub modifiers: TEEH<(usize, Modifiers)>,
    pub enter: TEEH<TestEnterEvent>,
    pub leave: TEEH<Leave>,
    pub event_id: NumCell<usize>,
}

impl TestKeyboard {
    pub fn destroy(&self) -> TestResult {
        if self.destroyed.set() {
            self.tran.send(Release { self_id: self.id })?;
        }
        Ok(())
    }

    fn handle_keymap(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Keymap::parse_full(parser)?;
        self.keymap.push((self.event_id.fetch_add(1), ev));
        Ok(())
    }

    fn handle_enter(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Enter::parse_full(parser)?;
        self.enter.push(TestEnterEvent {
            serial: ev.serial,
            surface: ev.surface,
            keys: ev.keys.to_vec(),
        });
        Ok(())
    }

    fn handle_leave(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Leave::parse_full(parser)?;
        self.leave.push(ev);
        Ok(())
    }

    fn handle_key(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Key::parse_full(parser)?;
        self.key.push((self.event_id.fetch_add(1), ev));
        Ok(())
    }

    fn handle_modifiers(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Modifiers::parse_full(parser)?;
        self.modifiers.push((self.event_id.fetch_add(1), ev));
        Ok(())
    }

    fn handle_repeat_info(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = RepeatInfo::parse_full(parser)?;
        Ok(())
    }
}

impl Drop for TestKeyboard {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestKeyboard, WlKeyboard;

    KEYMAP => handle_keymap,
    ENTER => handle_enter,
    LEAVE => handle_leave,
    KEY => handle_key,
    MODIFIERS => handle_modifiers,
    REPEAT_INFO => handle_repeat_info,
}

impl TestObject for TestKeyboard {}
