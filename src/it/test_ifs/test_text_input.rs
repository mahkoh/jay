use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_seat::TestSeat;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::wire::ZwpTextInputV3Id;
use crate::wire::zwp_text_input_v3::*;
use std::rc::Rc;

pub struct TestTextInput {
    pub id: ZwpTextInputV3Id,
    pub client: Rc<Client>,
    pub enter: TEEH<Enter>,
    pub leave: TEEH<Leave>,
    pub commit_string: TEEH<String>,
    pub done: TEEH<Done>,
}

impl TestTextInput {
    pub fn enable(&self) {
        self.client.send_zwp_text_input_v3_enable(self.id);
    }

    pub fn disable(&self) {
        self.client.send_zwp_text_input_v3_disable(self.id);
    }

    pub fn set_cursor_rectangle(&self, x: i32, y: i32, width: i32, height: i32) {
        self.client
            .send_zwp_text_input_v3_set_cursor_rectangle(self.id, x, y, width, height);
    }

    pub fn commit(&self) {
        self.client.send_zwp_text_input_v3_commit(self.id);
    }
}

synthetic_event_handler!(TestTextInput);

impl ZwpTextInputV3EventHandler for TestTextInput {
    type Error = TestErrorError;

    fn enter(&self, ev: Enter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.enter.push(ev);
        Ok(())
    }

    fn leave(&self, ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.leave.push(ev);
        Ok(())
    }

    fn preedit_string(&self, _ev: PreeditString<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn commit_string(&self, ev: CommitString<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.commit_string
            .push(ev.text.unwrap_or_default().to_string());
        Ok(())
    }

    fn delete_surrounding_text(
        &self,
        _ev: DeleteSurroundingText,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn done(&self, ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.done.push(ev);
        Ok(())
    }
}

impl TestClient {
    pub fn get_text_input(&self, seat: &TestSeat) -> Rc<TestTextInput> {
        let client = &self.client;
        let id = client.send_zwp_text_input_manager_v3_get_text_input(seat.id);
        let ti = Rc::new(TestTextInput {
            id,
            client: client.clone(),
            enter: Rc::new(Default::default()),
            leave: Rc::new(Default::default()),
            commit_string: Rc::new(Default::default()),
            done: Rc::new(Default::default()),
        });
        client.set_synthetic_event_handler(id, &ti);
        ti
    }
}
