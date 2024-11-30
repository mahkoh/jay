use {
    crate::{
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_surface::{
                tray::{
                    ack_configure, destroy, get_popup, install, DynTrayItem, FocusHint, Popup,
                    TrayItem, TrayItemData, TrayItemError,
                },
                WlSurface,
            },
            xdg_positioner::{ANCHOR_BOTTOM_LEFT, ANCHOR_BOTTOM_RIGHT},
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::NodeVisitor,
        utils::copyhashmap::CopyHashMap,
        wire::{ext_tray_item_v1::*, ExtTrayItemV1Id, XdgPopupId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtTrayItemV1 {
    id: ExtTrayItemV1Id,
    pub tracker: Tracker<Self>,
    version: Version,
    data: TrayItemData,
    popups: CopyHashMap<XdgPopupId, Rc<Popup<Self>>>,
}

impl ExtTrayItemV1 {
    pub fn new(
        id: ExtTrayItemV1Id,
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

    pub fn install(self: &Rc<Self>) -> Result<(), ExtTrayItemV1Error> {
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
        self.data.client.event(PreferredAnchor {
            self_id: self.id,
            anchor: ANCHOR_BOTTOM_LEFT,
        });
    }

    fn send_preferred_gravity(&self) {
        self.data.client.event(PreferredGravity {
            self_id: self.id,
            gravity: ANCHOR_BOTTOM_RIGHT,
        });
    }

    fn send_configure(&self) {
        self.data.client.event(Configure {
            self_id: self.id,
            serial: self.data.sent_serial.add_fetch(1),
        });
    }
}

impl ExtTrayItemV1RequestHandler for ExtTrayItemV1 {
    type Error = ExtTrayItemV1Error;

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
            n => return Err(ExtTrayItemV1Error::InvalidFocusHint(n)),
        };
        get_popup(slf, req.popup, req.seat, req.serial, focus)?;
        Ok(())
    }
}

impl TrayItem for ExtTrayItemV1 {
    fn send_initial_configure(&self) {
        self.send_preferred_anchor();
        self.send_preferred_gravity();
        <Self as TrayItem>::send_current_configure(self);
    }

    fn send_current_configure(&self) {
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
        visitor.visit_ext_tray_item(&self);
    }
}

object_base! {
    self = ExtTrayItemV1;
    version = self.version;
}

impl Object for ExtTrayItemV1 {
    fn break_loops(&self) {
        self.destroy_node();
    }
}

simple_add_obj!(ExtTrayItemV1);

#[derive(Debug, Error)]
pub enum ExtTrayItemV1Error {
    #[error(transparent)]
    TrayItemError(#[from] TrayItemError),
    #[error("The focus hint {} is invalid", .0)]
    InvalidFocusHint(u32),
}
