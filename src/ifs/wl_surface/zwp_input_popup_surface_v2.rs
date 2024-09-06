use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            wl_seat::text_input::zwp_input_method_v2::ZwpInputMethodV2,
            wl_surface::{SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError},
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        state::State,
        wire::{zwp_input_popup_surface_v2::*, WlSurfaceId, ZwpInputPopupSurfaceV2Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwpInputPopupSurfaceV2 {
    pub id: ZwpInputPopupSurfaceV2Id,
    pub client: Rc<Client>,
    pub input_method: Rc<ZwpInputMethodV2>,
    pub surface: Rc<WlSurface>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub positioning_scheduled: Cell<bool>,
}

impl SurfaceExt for ZwpInputPopupSurfaceV2 {
    fn after_apply_commit(self: Rc<Self>) {
        self.update_visible();
        if self.surface.visible.get() {
            self.schedule_positioning();
        }
    }
}

pub async fn input_popup_positioning(state: Rc<State>) {
    loop {
        let popup = state.pending_input_popup_positioning.pop().await;
        if popup.positioning_scheduled.get() {
            popup.position();
        }
    }
}

impl ZwpInputPopupSurfaceV2 {
    fn damage(&self) {
        let (x, y) = self.surface.buffer_abs_pos.get().position();
        let extents = self.surface.extents.get();
        self.client.state.damage(extents.move_(x, y));
    }

    pub fn update_visible(self: &Rc<Self>) {
        let was_visible = self.surface.visible.get();
        let is_visible = self.surface.buffer.is_some()
            && self.input_method.connection.is_some()
            && self.client.state.root_visible();
        self.surface.set_visible(is_visible);
        if was_visible != is_visible {
            if is_visible {
                self.schedule_positioning();
            } else {
                self.damage();
            }
        }
    }

    pub fn schedule_positioning(self: &Rc<Self>) {
        if self.surface.visible.get() {
            if !self.positioning_scheduled.replace(true) {
                self.client
                    .state
                    .pending_input_popup_positioning
                    .push(self.clone());
            }
        }
    }

    fn position(&self) {
        self.positioning_scheduled.set(false);
        if !self.surface.visible.get() {
            return;
        }
        let Some(con) = self.input_method.connection.get() else {
            log::warn!("Popup has no connection but is visible");
            return;
        };
        let output = con.surface.output.get().global.pos.get();
        let surface_rect = con.surface.buffer_abs_pos.get();
        let cursor_rect = con
            .text_input
            .cursor_rect()
            .move_(surface_rect.x1(), surface_rect.y1());
        let extents = self.surface.extents.get();
        let mut rect = extents.at_point(cursor_rect.x1(), cursor_rect.y2());
        let overflow = output.get_overflow(&rect);
        if overflow.right > 0 {
            let dx = -overflow.right.min(rect.width());
            let rect2 = rect.move_(dx, 0);
            if !output.get_overflow(&rect2).x_overflow() {
                rect = rect2;
            }
        }
        if overflow.bottom > 0 {
            let rect2 = rect.move_(0, -(cursor_rect.height() + rect.height()));
            if !output.get_overflow(&rect2).y_overflow() {
                rect = rect2;
            }
        }
        self.surface.buffer_abs_pos.set(
            self.surface
                .buffer_abs_pos
                .get()
                .at_point(rect.x1() - extents.x1(), rect.y1() - extents.y1()),
        );
    }

    pub fn install(self: &Rc<Self>) -> Result<(), ZwpInputPopupSurfaceV2Error> {
        self.surface.set_role(SurfaceRole::InputPopup)?;
        if self.surface.ext.get().is_some() {
            return Err(ZwpInputPopupSurfaceV2Error::AlreadyAttached(
                self.surface.id,
            ));
        }
        self.surface.ext.set(self.clone());
        self.input_method.popups.insert(self.id, self.clone());
        Ok(())
    }

    #[expect(dead_code)]
    pub fn send_text_input_rectangle(&self, rect: Rect) {
        self.client.event(TextInputRectangle {
            self_id: self.id,
            x: rect.x1(),
            y: rect.y1(),
            width: rect.width(),
            height: rect.height(),
        });
    }

    fn detach(&self) {
        if self.surface.visible.get() {
            self.damage();
        }
        self.surface.destroy_node();
        self.surface.unset_ext();
        self.input_method.popups.remove(&self.id);
    }
}

impl ZwpInputPopupSurfaceV2RequestHandler for ZwpInputPopupSurfaceV2 {
    type Error = ZwpInputPopupSurfaceV2Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpInputPopupSurfaceV2;
    version = self.version;
}

impl Object for ZwpInputPopupSurfaceV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpInputPopupSurfaceV2);

#[derive(Debug, Error)]
pub enum ZwpInputPopupSurfaceV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
    #[error("Surface {0} cannot be turned into a zwp_input_popup_surface_v2 because it already has an attached zwp_input_popup_surface_v2")]
    AlreadyAttached(WlSurfaceId),
}
efrom!(ZwpInputPopupSurfaceV2Error, WlSurfaceError);
efrom!(ZwpInputPopupSurfaceV2Error, ClientError);
