use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_input_method_keyboard_grab::TestInputMethodKeyboardGrab;
use crate::it::test_ifs::test_input_popup_surface::TestInputPopupSurface;
use crate::it::test_ifs::test_seat::TestSeat;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::utils::numcell::NumCell;
use crate::wire::ZwpInputMethodV2Id;
use crate::wire::zwp_input_method_v2::*;
use std::rc::Rc;

pub struct TestInputMethod {
    pub id: ZwpInputMethodV2Id,
    pub client: Rc<Client>,
    pub activate: TEEH<bool>,
    pub done: TEEH<()>,
    pub done_received: NumCell<u32>,
}

impl TestInputMethod {
    pub fn commit_string(&self, s: &str) {
        self.client
            .send_zwp_input_method_v2_commit_string(self.id, s);
    }

    pub fn commit(&self) {
        self.client
            .send_zwp_input_method_v2_commit(self.id, self.done_received.get());
    }

    #[expect(unused)]
    pub fn grab(&self) -> Rc<TestInputMethodKeyboardGrab> {
        let client = &self.client;
        let id = client.send_zwp_input_method_v2_grab_keyboard(self.id);
        let obj = Rc::new(TestInputMethodKeyboardGrab::default());
        client.set_synthetic_event_handler(id, &obj);
        obj
    }

    pub fn get_popup(&self, surface: &TestSurface) -> Rc<TestInputPopupSurface> {
        let client = &self.client;
        let id = client.send_zwp_input_method_v2_get_input_popup_surface(self.id, surface.id);
        let obj = Rc::new(TestInputPopupSurface);
        client.set_synthetic_event_handler(id, &obj);
        obj
    }
}

synthetic_event_handler!(TestInputMethod);

impl ZwpInputMethodV2EventHandler for TestInputMethod {
    type Error = TestErrorError;

    fn activate(&self, _ev: Activate, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.activate.push(true);
        Ok(())
    }

    fn deactivate(&self, _ev: Deactivate, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.activate.push(false);
        Ok(())
    }

    fn surrounding_text(
        &self,
        _ev: SurroundingText<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn text_change_cause(&self, _ev: TextChangeCause, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn content_type(&self, _ev: ContentType, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.done.push(());
        self.done_received.fetch_add(1);
        Ok(())
    }

    fn unavailable(&self, _ev: Unavailable, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl TestClient {
    pub fn get_input_method(&self, seat: &TestSeat) -> Rc<TestInputMethod> {
        let client = &self.client;
        let id = client.send_zwp_input_method_manager_v2_get_input_method(seat.id);
        let im = Rc::new(TestInputMethod {
            id,
            client: client.clone(),
            activate: Rc::new(Default::default()),
            done: Rc::new(Default::default()),
            done_received: Default::default(),
        });
        client.set_synthetic_event_handler(id, &im);
        im
    }
}
