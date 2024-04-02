use {
    crate::{
        ifs::wl_seat::wl_pointer::WlPointer,
        it::{
            test_error::TestResult, test_ifs::test_surface::TestSurface, test_object::TestObject,
            test_transport::TestTransport, test_utils::test_expected_event::TEEH,
            testrun::ParseFull,
        },
        utils::{buffd::MsgParser, clonecell::CloneCell},
        wire::{wl_pointer::*, WlPointerId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestPointer {
    pub id: WlPointerId,
    pub tran: Rc<TestTransport>,
    pub server: CloneCell<Option<Rc<WlPointer>>>,
    pub destroyed: Cell<bool>,
    pub leave: TEEH<Leave>,
    pub enter: TEEH<Enter>,
    pub motion: TEEH<Motion>,
    pub button: TEEH<Button>,
    pub axis_relative_direction: TEEH<AxisRelativeDirection>,
}

impl TestPointer {
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Release { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn set_cursor(
        &self,
        serial: u32,
        surface: &TestSurface,
        hotspot_x: i32,
        hotspot_y: i32,
    ) -> TestResult {
        self.tran.send(SetCursor {
            self_id: self.id,
            serial,
            surface: surface.id,
            hotspot_x,
            hotspot_y,
        })?;
        Ok(())
    }

    fn handle_enter(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Enter::parse_full(parser)?;
        self.enter.push(ev);
        Ok(())
    }

    fn handle_leave(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Leave::parse_full(parser)?;
        self.leave.push(ev);
        Ok(())
    }

    fn handle_motion(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Motion::parse_full(parser)?;
        self.motion.push(ev);
        Ok(())
    }

    fn handle_button(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = Button::parse_full(parser)?;
        self.button.push(ev);
        Ok(())
    }

    fn handle_axis(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Axis::parse_full(parser)?;
        Ok(())
    }

    fn handle_frame(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Frame::parse_full(parser)?;
        Ok(())
    }

    fn handle_axis_source(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = AxisSource::parse_full(parser)?;
        Ok(())
    }

    fn handle_axis_stop(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = AxisStop::parse_full(parser)?;
        Ok(())
    }

    fn handle_axis_discrete(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = AxisDiscrete::parse_full(parser)?;
        Ok(())
    }

    fn handle_axis_relative_direction(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = AxisRelativeDirection::parse_full(parser)?;
        self.axis_relative_direction.push(ev);
        Ok(())
    }
}

impl Drop for TestPointer {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestPointer, WlPointer;

    ENTER => handle_enter,
    LEAVE => handle_leave,
    MOTION => handle_motion,
    BUTTON => handle_button,
    AXIS => handle_axis,
    FRAME => handle_frame,
    AXIS_SOURCE => handle_axis_source,
    AXIS_STOP => handle_axis_stop,
    AXIS_DISCRETE => handle_axis_discrete,
    AXIS_RELATIVE_DIRECTION => handle_axis_relative_direction,
}

impl TestObject for TestPointer {}
