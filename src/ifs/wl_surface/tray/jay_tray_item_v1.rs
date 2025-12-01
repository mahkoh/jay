use {
    crate::{
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_surface::{
                WlSurface,
                tray::{
                    DynTrayItem, FocusHint, Popup, TrayItem, TrayItemData, TrayItemError,
                    ack_configure, destroy, get_popup, install,
                },
            },
            xdg_positioner::{
                ANCHOR_BOTTOM_LEFT, ANCHOR_BOTTOM_RIGHT, ANCHOR_TOP_LEFT, ANCHOR_TOP_RIGHT,
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::NodeVisitor,
        utils::copyhashmap::CopyHashMap,
        wire::{JayTrayItemV1Id, XdgPopupId, jay_tray_item_v1::*},
    },
    jay_config::theme::BarPosition,
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayTrayItemV1 {
    id: JayTrayItemV1Id,
    pub tracker: Tracker<Self>,
    version: Version,
    data: TrayItemData,
    popups: CopyHashMap<XdgPopupId, Rc<Popup<Self>>>,
}

impl JayTrayItemV1 {
    pub fn new(
        id: JayTrayItemV1Id,
        version: Version,
        surface: &Rc<WlSurface>,
        output: &Rc<OutputGlobalOpt>,
    ) -> Self {
        Self {
            id,
            tracker: Default::default(),
            version,
            popups: Default::default(),
            data: TrayItemData::new(surface, output),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), JayTrayItemV1Error> {
        install(self)?;
        Ok(())
    }

    fn send_configure_size(&self, width: i32, height: i32) {
        self.data.client.event(ConfigureSize {
            self_id: self.id,
            width,
            height,
        });
    }

    fn send_preferred_anchor(&self) {
        let anchor = match self.data.client.state.theme.bar_position.get() {
            BarPosition::Bottom => ANCHOR_TOP_RIGHT,
            BarPosition::Top | _ => ANCHOR_BOTTOM_RIGHT,
        };
        self.data.client.event(PreferredAnchor {
            self_id: self.id,
            anchor,
        });
    }

    fn send_preferred_gravity(&self) {
        let gravity = match self.data.client.state.theme.bar_position.get() {
            BarPosition::Bottom => ANCHOR_TOP_LEFT,
            BarPosition::Top | _ => ANCHOR_BOTTOM_LEFT,
        };
        self.data.client.event(PreferredGravity {
            self_id: self.id,
            gravity,
        });
    }

    fn send_configure(&self) {
        self.data.client.event(Configure {
            self_id: self.id,
            serial: self.data.sent_serial.add_fetch(1),
        });
    }
}

impl JayTrayItemV1RequestHandler for JayTrayItemV1 {
    type Error = JayTrayItemV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        destroy(self)?;
        Ok(())
    }

    fn ack_configure(&self, req: AckConfigure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        ack_configure(self, req.serial);
        Ok(())
    }

    fn get_popup(&self, req: GetPopup, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let focus = match req.keyboard_focus {
            0 => FocusHint::None,
            1 => FocusHint::OnDemand,
            2 => FocusHint::Immediate,
            n => return Err(JayTrayItemV1Error::InvalidFocusHint(n)),
        };
        get_popup(slf, req.popup, req.seat, req.serial, focus)?;
        Ok(())
    }
}

impl TrayItem for JayTrayItemV1 {
    fn send_current_configure(&self) {
        self.send_preferred_anchor();
        self.send_preferred_gravity();
        let size = self.data.client.state.tray_icon_size().max(1);
        self.send_configure_size(size, size);
        self.send_configure();
    }

    fn data(&self) -> &TrayItemData {
        &self.data
    }

    fn popups(&self) -> &CopyHashMap<XdgPopupId, Rc<Popup<Self>>> {
        &self.popups
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_tray_item(&self);
    }
}

object_base! {
    self = JayTrayItemV1;
    version = self.version;
}

impl Object for JayTrayItemV1 {
    fn break_loops(&self) {
        self.destroy_node();
    }
}

simple_add_obj!(JayTrayItemV1);

#[derive(Debug, Error)]
pub enum JayTrayItemV1Error {
    #[error(transparent)]
    TrayItemError(#[from] TrayItemError),
    #[error("The focus hint {} is invalid", .0)]
    InvalidFocusHint(u32),
}
