use {
    crate::{
        ifs::wl_seat::wl_pointer::PendingScroll,
        object::Version,
        utils::clonecell::CloneCell,
        wire::{wl_pointer::*, WlPointerId},
        wl_usr::{usr_ifs::usr_wl_surface::UsrWlSurface, usr_object::UsrObject, UsrCon},
    },
    std::{cell::Cell, convert::Infallible, rc::Rc},
};

pub struct UsrWlPointer {
    pub id: WlPointerId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlPointerOwner>>>,
    pub any_scroll_events: Cell<bool>,
    pub pending_scroll: PendingScroll,
    pub version: Version,
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
}

impl WlPointerEventHandler for UsrWlPointer {
    type Error = Infallible;

    fn enter(&self, ev: Enter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.enter(&ev);
        }
        Ok(())
    }

    fn leave(&self, ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.leave(&ev);
        }
        Ok(())
    }

    fn motion(&self, ev: Motion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.motion(&ev);
        }
        Ok(())
    }

    fn button(&self, ev: Button, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.button(&ev);
        }
        Ok(())
    }

    fn axis(&self, ev: Axis, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending_scroll.time_usec.set(ev.time as u64 * 1000);
        if ev.axis < 2 {
            self.pending_scroll.px[ev.axis as usize].set(Some(ev.value));
        }
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn frame(&self, _ev: Frame, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.any_scroll_events.take() {
            let pe = self.pending_scroll.take();
            if let Some(owner) = self.owner.get() {
                owner.scroll(&pe);
            }
        }
        Ok(())
    }

    fn axis_source(&self, ev: AxisSource, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending_scroll.source.set(Some(ev.axis_source));
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn axis_stop(&self, ev: AxisStop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending_scroll.time_usec.set(ev.time as u64 * 1000);
        if ev.axis < 2 {
            self.pending_scroll.stop[ev.axis as usize].set(true);
        }
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn axis_discrete(&self, _ev: AxisDiscrete, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn axis_value120(&self, ev: AxisValue120, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if ev.axis < 2 {
            self.pending_scroll.v120[ev.axis as usize].set(Some(ev.value120));
        }
        self.any_scroll_events.set(true);
        Ok(())
    }

    fn axis_relative_direction(
        &self,
        _ev: AxisRelativeDirection,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlPointer = WlPointer;
    version = self.version;
}

impl UsrObject for UsrWlPointer {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
