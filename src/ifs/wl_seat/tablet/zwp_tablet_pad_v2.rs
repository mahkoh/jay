use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            wl_seat::tablet::{
                zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
                zwp_tablet_v2::ZwpTabletV2, PadButtonState, TabletPad,
            },
            wl_surface::WlSurface,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_tablet_pad_v2::*, ZwpTabletPadV2Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwpTabletPadV2 {
    pub id: ZwpTabletPadV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub seat: Rc<ZwpTabletSeatV2>,
    pub pad: Rc<TabletPad>,
    pub entered: Cell<bool>,
}

impl ZwpTabletPadV2 {
    pub fn detach(&self) {
        self.pad.bindings.remove(&self.seat);
    }

    pub fn send_group(&self, group: &ZwpTabletPadGroupV2) {
        self.client.event(Group {
            self_id: self.id,
            pad_group: group.id,
        });
    }

    pub fn send_path(&self, path: &str) {
        self.client.event(Path {
            self_id: self.id,
            path,
        });
    }

    pub fn send_buttons(&self, buttons: u32) {
        self.client.event(Buttons {
            self_id: self.id,
            buttons,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_button(&self, time: u32, button: u32, state: PadButtonState) {
        self.client.event(Button {
            self_id: self.id,
            time,
            button,
            state: match state {
                PadButtonState::Released => 0,
                PadButtonState::Pressed => 1,
            },
        });
    }

    pub fn send_enter(&self, serial: u64, tablet: &ZwpTabletV2, surface: &WlSurface) {
        self.entered.set(true);
        self.client.event(Enter {
            self_id: self.id,
            serial: serial as _,
            tablet: tablet.id,
            surface: surface.id,
        });
    }

    pub fn send_leave(&self, serial: u64, surface: &WlSurface) {
        self.entered.set(false);
        self.client.event(Leave {
            self_id: self.id,
            serial: serial as _,
            surface: surface.id,
        });
    }

    pub fn send_removed(&self) {
        self.client.event(Removed { self_id: self.id });
    }
}

impl ZwpTabletPadV2RequestHandler for ZwpTabletPadV2 {
    type Error = ZwpTabletPadV2Error;

    fn set_feedback(&self, _req: SetFeedback<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpTabletPadV2;
    version = self.version;
}

impl Object for ZwpTabletPadV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpTabletPadV2);

#[derive(Debug, Error)]
pub enum ZwpTabletPadV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTabletPadV2Error, ClientError);
