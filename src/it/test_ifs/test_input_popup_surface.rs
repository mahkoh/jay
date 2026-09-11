use crate::it::test_error::TestErrorError;
use crate::wire::zwp_input_popup_surface_v2::*;
use std::rc::Rc;

pub struct TestInputPopupSurface;

synthetic_event_handler!(TestInputPopupSurface);

impl ZwpInputPopupSurfaceV2EventHandler for TestInputPopupSurface {
    type Error = TestErrorError;

    fn text_input_rectangle(
        &self,
        _ev: TextInputRectangle,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
