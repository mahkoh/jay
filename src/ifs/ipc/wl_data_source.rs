use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                add_data_source_mime_type, break_source_loops, cancel_offers, destroy_data_source,
                detach_seat, offer_source_to_regular_client, offer_source_to_wlr_device,
                offer_source_to_x,
                wl_data_device::ClipboardIpc,
                wl_data_device_manager::{DND_ALL, DND_NONE},
                x_data_device::{XClipboardIpc, XIpcDevice},
                zwlr_data_control_device_v1::{WlrClipboardIpc, ZwlrDataControlDeviceV1},
                DataSource, DynDataOffer, DynDataSource, SharedState, SourceData,
                OFFER_STATE_ACCEPTED, OFFER_STATE_DROPPED, SOURCE_STATE_CANCELLED,
                SOURCE_STATE_DROPPED,
            },
            wl_seat::WlSeatGlobal,
            wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_drag_v1::XdgToplevelDragV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::{bitflags::BitflagsExt, cell_ext::CellExt, clonecell::CloneCell},
        wire::{wl_data_source::*, WlDataSourceId},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

#[expect(dead_code)]
const INVALID_ACTION_MASK: u32 = 0;
#[expect(dead_code)]
const INVALID_SOURCE: u32 = 1;

pub struct WlDataSource {
    pub id: WlDataSourceId,
    pub data: SourceData,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub toplevel_drag: CloneCell<Option<Rc<XdgToplevelDragV1>>>,
}

impl DataSource for WlDataSource {
    fn send_cancelled(&self, seat: &Rc<WlSeatGlobal>) {
        WlDataSource::send_cancelled(self, seat);
    }
}

impl DynDataSource for WlDataSource {
    fn source_data(&self) -> &SourceData {
        &self.data
    }

    fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        WlDataSource::send_send(self, mime_type, fd);
    }

    fn offer_to_regular_client(self: Rc<Self>, client: &Rc<Client>) {
        offer_source_to_regular_client::<ClipboardIpc, Self>(&self, client);
    }

    fn offer_to_x(self: Rc<Self>, dd: &Rc<XIpcDevice>) {
        offer_source_to_x::<XClipboardIpc, Self>(&self, dd);
    }

    fn offer_to_wlr_device(self: Rc<Self>, dd: &Rc<ZwlrDataControlDeviceV1>) {
        offer_source_to_wlr_device::<WlrClipboardIpc, Self>(&self, dd)
    }

    fn detach_seat(&self, seat: &Rc<WlSeatGlobal>) {
        detach_seat(self, seat);
    }

    fn cancel_unprivileged_offers(&self) {
        cancel_offers(self, false);
    }

    fn send_target(&self, mime_type: Option<&str>) {
        WlDataSource::send_target(self, mime_type);
    }

    fn send_dnd_finished(&self) {
        WlDataSource::send_dnd_finished(self);
    }

    fn update_selected_action(&self) {
        WlDataSource::update_selected_action(self);
    }
}

impl WlDataSource {
    pub fn new(id: WlDataSourceId, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            tracker: Default::default(),
            data: SourceData::new(client),
            version,
            toplevel_drag: Default::default(),
        }
    }

    pub fn on_leave(&self) {
        if self
            .data
            .shared
            .get()
            .state
            .get()
            .contains(OFFER_STATE_DROPPED)
        {
            return;
        }
        self.data.shared.set(Rc::new(SharedState::default()));
        self.send_target(None);
        self.send_action(DND_NONE);
        cancel_offers(self, false);
    }

    pub fn update_selected_action(&self) {
        let shared = self.data.shared.get();
        let server_actions = match self.data.actions.get() {
            Some(n) => n,
            _ => {
                log::error!("Server actions not set");
                return;
            }
        };
        let actions = server_actions & shared.receiver_actions.get();
        let action = if actions.contains(shared.receiver_preferred_action.get()) {
            shared.receiver_preferred_action.get()
        } else if actions != 0 {
            1 << actions.trailing_zeros()
        } else {
            0
        };
        if shared.selected_action.replace(action) != action {
            for (_, offer) in &self.data.offers {
                offer.send_action(action);
                // offer.client.flush();
            }
            self.send_action(action);
            // self.data.client.flush();
        }
    }

    pub fn for_each_data_offer<C: FnMut(&dyn DynDataOffer)>(&self, mut f: C) {
        for (_, offer) in &self.data.offers {
            f(&*offer);
        }
    }

    pub fn can_drop(&self) -> bool {
        let shared = self.data.shared.get();
        shared.selected_action.get() != 0 && shared.state.get().contains(OFFER_STATE_ACCEPTED)
    }

    pub fn on_drop(&self) {
        self.data.state.or_assign(SOURCE_STATE_DROPPED);
        self.send_dnd_drop_performed();
        let shared = self.data.shared.get();
        shared.state.or_assign(OFFER_STATE_DROPPED);
    }

    pub fn finish_toplevel_drag(&self, seat: &Rc<WlSeatGlobal>) {
        if let Some(drag) = self.toplevel_drag.take() {
            drag.finish_drag(seat);
        }
    }

    pub fn send_cancelled(&self, seat: &Rc<WlSeatGlobal>) {
        self.data.state.or_assign(SOURCE_STATE_CANCELLED);
        if let Some(drag) = self.toplevel_drag.take() {
            drag.finish_drag(seat);
        }
        self.data.client.event(Cancelled { self_id: self.id })
    }

    pub fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.data.client.event(Send {
            self_id: self.id,
            mime_type,
            fd,
        })
    }

    pub fn send_target(&self, mime_type: Option<&str>) {
        self.data.client.event(Target {
            self_id: self.id,
            mime_type,
        })
    }

    pub fn send_dnd_finished(&self) {
        self.data.client.event(DndFinished { self_id: self.id })
    }

    pub fn send_action(&self, dnd_action: u32) {
        self.data.client.event(Action {
            self_id: self.id,
            dnd_action,
        })
    }

    pub fn send_dnd_drop_performed(&self) {
        self.data
            .client
            .event(DndDropPerformed { self_id: self.id })
    }
}

impl WlDataSourceRequestHandler for WlDataSource {
    type Error = WlDataSourceError;

    fn offer(&self, req: Offer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        add_data_source_mime_type::<ClipboardIpc>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        destroy_data_source::<ClipboardIpc>(self);
        self.data.client.remove_obj(self)?;
        Ok(())
    }

    fn set_actions(&self, req: SetActions, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.data.actions.is_some() {
            return Err(WlDataSourceError::AlreadySet);
        }
        if req.dnd_actions & !DND_ALL != 0 {
            return Err(WlDataSourceError::InvalidActions);
        }
        self.data.actions.set(Some(req.dnd_actions));
        Ok(())
    }
}

object_base! {
    self = WlDataSource;
    version = self.version;
}

impl Object for WlDataSource {
    fn break_loops(&self) {
        break_source_loops::<ClipboardIpc>(self);
        self.toplevel_drag.take();
    }
}

dedicated_add_obj!(WlDataSource, WlDataSourceId, wl_data_source);

#[derive(Debug, Error)]
pub enum WlDataSourceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The set of actions is invalid")]
    InvalidActions,
    #[error("The actions have already been set")]
    AlreadySet,
}
efrom!(WlDataSourceError, ClientError);
