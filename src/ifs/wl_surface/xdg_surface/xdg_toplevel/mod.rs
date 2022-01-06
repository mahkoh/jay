mod types;

use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::ifs::wl_surface::{RoleData, XdgSurfaceRoleData};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use num_derive::FromPrimitive;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const SET_PARENT: u32 = 1;
const SET_TITLE: u32 = 2;
const SET_APP_ID: u32 = 3;
const SHOW_WINDOW_MENU: u32 = 4;
const MOVE: u32 = 5;
const RESIZE: u32 = 6;
const SET_MAX_SIZE: u32 = 7;
const SET_MIN_SIZE: u32 = 8;
const SET_MAXIMIZED: u32 = 9;
const UNSET_MAXIMIZED: u32 = 10;
const SET_FULLSCREEN: u32 = 11;
const UNSET_FULLSCREEN: u32 = 12;
const SET_MINIMIZED: u32 = 13;

const CONFIGURE: u32 = 0;
const CLOSE: u32 = 1;

#[derive(Copy, Clone, Debug, FromPrimitive)]
pub enum ResizeEdge {
    None = 0,
    Top = 1,
    Bottom = 2,
    Left = 4,
    TopLeft = 5,
    BottomLeft = 6,
    Right = 8,
    TopRight = 9,
    BottomRight = 10,
}

const STATE_MAXIMIZED: u32 = 1;
const STATE_FULLSCREEN: u32 = 2;
const STATE_RESIZING: u32 = 3;
const STATE_ACTIVATED: u32 = 4;
const STATE_TILED_LEFT: u32 = 5;
const STATE_TILED_RIGHT: u32 = 6;
const STATE_TILED_TOP: u32 = 7;
const STATE_TILED_BOTTOM: u32 = 8;

id!(XdgToplevelId);

pub struct XdgToplevel {
    id: XdgToplevelId,
    pub surface: Rc<XdgSurface>,
    version: u32,
}

impl XdgToplevel {
    pub fn new(id: XdgToplevelId, surface: &Rc<XdgSurface>, version: u32) -> Self {
        Self {
            id,
            surface: surface.clone(),
            version,
        }
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.surface.client.parse(self, parser)?;
        {
            let mut rd = self.surface.surface.role_data.borrow_mut();
            if let RoleData::XdgSurface(rd) = &mut *rd {
                rd.role_data = XdgSurfaceRoleData::None;
            }
        }
        Ok(())
    }

    async fn set_parent(&self, parser: MsgParser<'_, '_>) -> Result<(), SetParentError> {
        let _req: SetParent = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_title(&self, parser: MsgParser<'_, '_>) -> Result<(), SetTitleError> {
        let _req: SetTitle = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_app_id(&self, parser: MsgParser<'_, '_>) -> Result<(), SetAppIdError> {
        let _req: SetAppId = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn show_window_menu(&self, parser: MsgParser<'_, '_>) -> Result<(), ShowWindowMenuError> {
        let _req: ShowWindowMenu = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn move_(&self, parser: MsgParser<'_, '_>) -> Result<(), MoveError> {
        let _req: Move = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn resize(&self, parser: MsgParser<'_, '_>) -> Result<(), ResizeError> {
        let _req: Resize = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_max_size(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMaxSizeError> {
        let _req: SetMaxSize = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_min_size(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMinSizeError> {
        let _req: SetMinSize = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_maximized(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMaximizedError> {
        let _req: SetMaximized = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn unset_maximized(&self, parser: MsgParser<'_, '_>) -> Result<(), UnsetMaximizedError> {
        let _req: UnsetMaximized = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_fullscreen(&self, parser: MsgParser<'_, '_>) -> Result<(), SetFullscreenError> {
        let _req: SetFullscreen = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn unset_fullscreen(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), UnsetFullscreenError> {
        let _req: UnsetFullscreen = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_minimized(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMinimizedError> {
        let _req: SetMinimized = self.surface.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgToplevelError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            SET_PARENT => self.set_parent(parser).await?,
            SET_TITLE => self.set_title(parser).await?,
            SET_APP_ID => self.set_app_id(parser).await?,
            SHOW_WINDOW_MENU => self.show_window_menu(parser).await?,
            MOVE => self.move_(parser).await?,
            RESIZE => self.resize(parser).await?,
            SET_MAX_SIZE => self.set_max_size(parser).await?,
            SET_MIN_SIZE => self.set_min_size(parser).await?,
            SET_MAXIMIZED => self.set_maximized(parser).await?,
            UNSET_MAXIMIZED => self.unset_maximized(parser).await?,
            SET_FULLSCREEN => self.set_fullscreen(parser).await?,
            UNSET_FULLSCREEN => self.unset_fullscreen(parser).await?,
            SET_MINIMIZED => self.set_minimized(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(XdgToplevel);

impl Object for XdgToplevel {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::XdgToplevel
    }

    fn num_requests(&self) -> u32 {
        SET_MINIMIZED + 1
    }
}
