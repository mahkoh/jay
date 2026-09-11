use crate::client::Client;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_keyboard::TestKeyboard;
use crate::it::test_ifs::test_pointer::TestPointer;
use crate::utils::clonecell::CloneCell;
use crate::wire::WlSeatId;
use crate::wire::wl_seat::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestSeat {
    pub id: WlSeatId,
    pub client: Rc<Client>,
    pub caps: Cell<u32>,
    pub name: CloneCell<Option<Rc<String>>>,
}

impl TestSeat {
    pub fn get_keyboard(&self) -> Rc<TestKeyboard> {
        let id = self.client.send_wl_seat_get_keyboard(self.id);
        let kb = Rc::new(TestKeyboard {
            keymap: Default::default(),
            key: Default::default(),
            modifiers: Default::default(),
            enter: Default::default(),
            leave: Default::default(),
            event_id: Default::default(),
        });
        self.client.set_synthetic_event_handler(id, &kb);
        kb
    }

    pub fn get_pointer(&self) -> Rc<TestPointer> {
        let id = self.client.send_wl_seat_get_pointer(self.id);
        let pointer = Rc::new(TestPointer {
            id,
            client: self.client.clone(),
            leave: Default::default(),
            enter: Default::default(),
            motion: Default::default(),
            button: Default::default(),
            axis_relative_direction: Default::default(),
        });
        self.client.set_synthetic_event_handler(id, &pointer);
        pointer
    }
}

synthetic_event_handler!(TestSeat);

impl WlSeatEventHandler for TestSeat {
    type Error = TestErrorError;

    fn capabilities(&self, ev: Capabilities, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.caps.set(ev.capabilities);
        Ok(())
    }

    fn name(&self, ev: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.name.set(Some(Rc::new(ev.name.to_string())));
        Ok(())
    }
}
