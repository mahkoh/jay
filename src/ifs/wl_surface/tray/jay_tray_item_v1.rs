use crate::configurable::Configurable;
use crate::configurable::ConfigurableData;
use crate::ifs::wl_output::OutputGlobalOpt;
use crate::ifs::wl_surface::WlSurface;
use crate::ifs::wl_surface::tray::DynTrayItem;
use crate::ifs::wl_surface::tray::FocusHint;
use crate::ifs::wl_surface::tray::Popup;
use crate::ifs::wl_surface::tray::TrayItem;
use crate::ifs::wl_surface::tray::TrayItemConfigureData;
use crate::ifs::wl_surface::tray::TrayItemData;
use crate::ifs::wl_surface::tray::TrayItemError;
use crate::ifs::wl_surface::tray::TrayItemTransactionOp;
use crate::ifs::wl_surface::tray::ack_configure;
use crate::ifs::wl_surface::tray::destroy;
use crate::ifs::wl_surface::tray::get_popup;
use crate::ifs::wl_surface::tray::install;
use crate::ifs::xdg_positioner::ANCHOR_BOTTOM_LEFT;
use crate::ifs::xdg_positioner::ANCHOR_BOTTOM_RIGHT;
use crate::ifs::xdg_positioner::ANCHOR_TOP_LEFT;
use crate::ifs::xdg_positioner::ANCHOR_TOP_RIGHT;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::theme::BarPosition;
use crate::transactions::TransactionData;
use crate::transactions::Transactionable;
use crate::tree::NodeVisitor;
use crate::tree::TreeSerial;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::copyhashmap::CopyHashMap;
use crate::wire::JayTrayItemV1Id;
use crate::wire::XdgPopupId;
use crate::wire::jay_tray_item_v1::*;
use std::rc::Rc;
use thiserror::Error;

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

    fn send_preferred_anchor(&self, bar_position: BarPosition) {
        let anchor = match bar_position {
            BarPosition::Bottom => ANCHOR_TOP_RIGHT,
            BarPosition::Top => ANCHOR_BOTTOM_RIGHT,
        };
        self.data.client.event(PreferredAnchor {
            self_id: self.id,
            anchor,
        });
    }

    fn send_preferred_gravity(&self, bar_position: BarPosition) {
        let gravity = match bar_position {
            BarPosition::Bottom => ANCHOR_TOP_LEFT,
            BarPosition::Top => ANCHOR_BOTTOM_LEFT,
        };
        self.data.client.event(PreferredGravity {
            self_id: self.id,
            gravity,
        });
    }

    fn send_configure(&self, serial: TreeSerial) {
        self.data.sent_serial.set(Some(serial));
        self.data.client.event(Configure {
            self_id: self.id,
            serial: serial.raw() as _,
        });
    }
}

impl JayTrayItemV1RequestHandler for JayTrayItemV1 {
    type Error = JayTrayItemV1Error;

    fn destroy(&self, _req: Destroy, slf: &Rc<Self>) -> Result<(), Self::Error> {
        destroy(slf)?;
        Ok(())
    }

    fn ack_configure(&self, req: AckConfigure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let serial = self.data.client.state.map_tree_serial32(req.serial);
        ack_configure(self, serial);
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
    fn tray_item_data(&self) -> &TrayItemData {
        &self.data
    }

    fn popups(&self) -> &CopyHashMap<XdgPopupId, Rc<Popup<Self>>> {
        &self.popups
    }

    fn visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_tray_item(self);
    }
}

object_base! {
    self = JayTrayItemV1;
    version = self.version;
}

impl Object for JayTrayItemV1 {
    fn break_loops(self: Rc<Self>) {
        self.clone().destroy_node();
        self.data.destroyed.set(true);
        self.data.configurable.ready();
    }
}

simple_add_obj!(JayTrayItemV1);

impl Configurable for JayTrayItemV1 {
    type T = TrayItemConfigureData;

    fn data(&self) -> &ConfigurableData<Self::T> {
        &self.data.configurable
    }

    fn configure_data(&self) -> Self::T {
        let state = &self.tray_item_data().client.state;
        let size = state.tray_icon_size().max(1);
        let bar_position = state.theme.bar_position[LiveTL].get();
        TrayItemConfigureData { size, bar_position }
    }

    fn merge(first: &mut Self::T, second: Self::T) {
        *first = second;
    }

    fn visible(&self) -> bool {
        self.data.visible.get()
    }

    fn destroyed(&self) -> bool {
        self.data.destroyed.get()
    }

    fn surface(&self) -> &Rc<WlSurface> {
        &self.data.surface
    }

    fn flush(&self, serial: TreeSerial, data: Self::T) {
        self.send_preferred_anchor(data.bar_position);
        self.send_preferred_gravity(data.bar_position);
        self.send_configure_size(data.size, data.size);
        self.send_configure(serial);
    }
}

impl Transactionable for JayTrayItemV1 {
    type T = TrayItemTransactionOp;

    fn data(&self) -> &TransactionData<Self::T> {
        &self.data.transaction_data
    }

    fn apply(self: &Rc<Self>, op: Self::T) {
        match op {
            TrayItemTransactionOp::SetValid(v) => {
                v.set_valid();
            }
            TrayItemTransactionOp::Unlink(v) => {
                drop(v);
            }
            TrayItemTransactionOp::SetRelPos(v) => {
                self.data.rel_pos[RenderTL].set(v);
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum JayTrayItemV1Error {
    #[error(transparent)]
    TrayItemError(#[from] TrayItemError),
    #[error("The focus hint {} is invalid", .0)]
    InvalidFocusHint(u32),
}
