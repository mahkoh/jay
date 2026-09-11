use crate::client::Client;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::wire::WlPointerId;
use crate::wire::WlSurfaceId;
use crate::wire::wl_pointer::*;
use std::rc::Rc;

pub struct TestPointer {
    pub id: WlPointerId,
    pub client: Rc<Client>,
    pub leave: TEEH<Leave>,
    pub enter: TEEH<Enter>,
    pub motion: TEEH<Motion>,
    pub button: TEEH<Button>,
    pub axis_relative_direction: TEEH<AxisRelativeDirection>,
}

impl TestPointer {
    pub fn set_cursor(
        &self,
        serial: u32,
        surface: Option<&TestSurface>,
        hotspot_x: i32,
        hotspot_y: i32,
    ) {
        self.client.send_wl_pointer_set_cursor(
            self.id,
            serial,
            surface.map(|s| s.id).unwrap_or(WlSurfaceId::NONE),
            hotspot_x,
            hotspot_y,
        );
    }
}

synthetic_event_handler!(TestPointer);

impl WlPointerEventHandler for TestPointer {
    type Error = TestErrorError;

    fn enter(&self, ev: Enter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.enter.push(ev);
        Ok(())
    }

    fn leave(&self, ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.leave.push(ev);
        Ok(())
    }

    fn motion(&self, ev: Motion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.motion.push(ev);
        Ok(())
    }

    fn button(&self, ev: Button, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.button.push(ev);
        Ok(())
    }

    fn axis(&self, _ev: Axis, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn frame(&self, _ev: Frame, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn axis_source(&self, _ev: AxisSource, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn axis_stop(&self, _ev: AxisStop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn axis_discrete(&self, _ev: AxisDiscrete, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn axis_value120(&self, _ev: AxisValue120, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn axis_relative_direction(
        &self,
        ev: AxisRelativeDirection,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.axis_relative_direction.push(ev);
        Ok(())
    }

    fn warp(&self, _ev: Warp, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}
