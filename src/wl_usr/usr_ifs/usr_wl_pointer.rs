use {
    crate::{
        ifs::wl_seat::wl_pointer::PendingScroll,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{wl_pointer::*, WlPointerId},
        wl_usr::{usr_ifs::usr_wl_surface::UsrWlSurface, usr_object::UsrObject, UsrCon},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct UsrWlPointer {
    pub id: WlPointerId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlPointerOwner>>>,
    pub any_scroll_events: Cell<bool>,
    pub pending_scroll: PendingScroll,
}

pub trait UsrWlPointerOwner {
    fn enter(&self, ev: &Enter) {
        let _ = ev;
    }

    fn leave(&self, ev: &Leave) {
        let _ = ev;
    }

    fn motion(&self, ev: &Motion) {
        let _ = ev;
    }

    fn button(&self, ev: &Button) {
        let _ = ev;
    }

    fn scroll(&self, ps: &PendingScroll) {
        let _ = ps;
    }
}

impl UsrWlPointer {
    #[allow(dead_code)]
    pub fn set_cursor(&self, serial: u32, cursor: &UsrWlSurface, hot_x: i32, hot_y: i32) {
        self.con.request(SetCursor {
            self_id: self.id,
            serial,
            surface: cursor.id,
            hotspot_x: hot_x,
            hotspot_y: hot_y,
        });
    }

    fn enter(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Enter = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.enter(&ev);
        }
        Ok(())
    }

    fn leave(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Leave = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.leave(&ev);
        }
        Ok(())
    }

    fn motion(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Motion = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.motion(&ev);
        }
        Ok(())
    }

    fn button(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Button = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.button(&ev);
        }
        Ok(())
    }

    fn axis(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Axis = self.con.parse(self, parser)?;
        self.pending_scroll.time_usec.set(ev.time as u64 * 1000);
        if ev.axis < 2 {
            self.pending_scroll.px[ev.axis as usize].set(Some(ev.value));
        }
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn frame(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Frame = self.con.parse(self, parser)?;
        if self.any_scroll_events.take() {
            let pe = self.pending_scroll.take();
            if let Some(owner) = self.owner.get() {
                owner.scroll(&pe);
            }
        }
        Ok(())
    }

    fn axis_source(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: AxisSource = self.con.parse(self, parser)?;
        self.pending_scroll.source.set(Some(ev.axis_source));
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn axis_stop(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: AxisStop = self.con.parse(self, parser)?;
        self.pending_scroll.time_usec.set(ev.time as u64 * 1000);
        if ev.axis < 2 {
            self.pending_scroll.stop[ev.axis as usize].set(true);
        }
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn axis_discrete(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: AxisDiscrete = self.con.parse(self, parser)?;
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn axis_value120(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: AxisValue120 = self.con.parse(self, parser)?;
        if ev.axis < 2 {
            self.pending_scroll.v120[ev.axis as usize].set(Some(ev.value120));
        }
        self.any_scroll_events.set(true);
        Ok(())
    }
}

usr_object_base! {
    UsrWlPointer, WlPointer;

    ENTER => enter,
    LEAVE => leave,
    MOTION => motion,
    BUTTON => button,
    AXIS => axis,
    FRAME => frame,
    AXIS_SOURCE => axis_source,
    AXIS_STOP => axis_stop,
    AXIS_DISCRETE => axis_discrete,
    AXIS_VALUE120 => axis_value120,
}

impl UsrObject for UsrWlPointer {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
