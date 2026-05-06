use {
    crate::{
        client::{Client, ClientError},
        gfx_api::GfxTexture,
        ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel,
        leaks::Tracker,
        object::{Object, Version},
        scale::Scale,
        state::State,
        theme::Color,
        utils::{
            clonecell::UnsafeCellCloneSafe, copyhashmap::CopyHashMap, obj_and_id::ObjWithId,
            smallmap::SmallMap,
        },
        wire::{XdgToplevelIconV1Id, XdgToplevelId, xdg_toplevel_icon_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

linear_ids!(ToplevelIconIds, ToplevelIconId, u64);

pub struct XdgToplevelIconV1 {
    pub id: XdgToplevelIconV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub immutable: Cell<bool>,
    pub toplevel_icon_id: ToplevelIconId,
    pub toplevels: CopyHashMap<XdgToplevelId, Rc<XdgToplevel>>,
}

pub struct ToplevelIconUser {
    size: Cell<i32>,
    icons: SmallMap<Scale, ToplevelIcon, 2>,
}

#[derive(Clone)]
pub enum ToplevelIcon {
    #[expect(dead_code)]
    Srgb(Color),
    #[expect(dead_code)]
    Tex(Rc<dyn GfxTexture>),
}

unsafe impl UnsafeCellCloneSafe for ToplevelIcon {}

impl ToplevelIconUser {
    pub fn new(size: i32) -> Self {
        Self {
            size: Cell::new(size),
            icons: Default::default(),
        }
    }

    pub fn clear(&self) {
        self.icons.clear();
    }

    pub fn set_size(&self, size: i32) -> bool {
        self.size.replace(size) != size
    }

    pub fn get(&self, scale: Scale) -> Option<ToplevelIcon> {
        self.icons.get(&scale)
    }
}

impl State {
    pub fn toplevel_icon_user(&self) -> ToplevelIconUser {
        ToplevelIconUser::new(self.theme.title_icon_size())
    }
}

impl ObjWithId for Rc<XdgToplevelIconV1> {
    type Id = ToplevelIconId;

    fn id(&self) -> Self::Id {
        self.toplevel_icon_id
    }
}

impl XdgToplevelIconV1 {
    pub fn new(id: XdgToplevelIconV1Id, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            immutable: Default::default(),
            toplevel_icon_id: client.state.toplevel_icon_ids.next(),
            toplevels: Default::default(),
        }
    }

    fn check_immutable(&self) -> Result<(), XdgToplevelIconV1Error> {
        if self.immutable.get() {
            return Err(XdgToplevelIconV1Error::Immutable);
        }
        Ok(())
    }

    pub fn handle_render_ctx_change(self: &Rc<Self>) {
        self.update_sizes();
    }

    pub fn update_sizes(self: &Rc<Self>) {
        if !self.immutable.get() {
            return;
        }
    }

    pub fn update_user(&self, user: &ToplevelIconUser) {
        user.icons.clear();
    }

    pub fn is_empty(&self) -> bool {
        true
    }

    pub fn has_no_pending(&self) -> bool {
        true
    }
}

impl Drop for XdgToplevelIconV1 {
    fn drop(&mut self) {
        self.client
            .state
            .toplevel_icons
            .remove(&self.toplevel_icon_id);
    }
}

impl XdgToplevelIconV1RequestHandler for XdgToplevelIconV1 {
    type Error = XdgToplevelIconV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_name(&self, _req: SetName<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_immutable()?;
        Ok(())
    }

    fn add_buffer(&self, req: AddBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_immutable()?;
        let buffer = self.client.lookup(req.buffer)?;
        if buffer.rect.width() != buffer.rect.height() {
            return Err(XdgToplevelIconV1Error::NotSquare);
        }
        Ok(())
    }
}

object_base! {
    self = XdgToplevelIconV1;
    version = self.version;
}

impl Object for XdgToplevelIconV1 {
    fn break_loops(&self) {
        self.toplevels.clear();
    }
}

dedicated_add_obj!(XdgToplevelIconV1, XdgToplevelIconV1Id, xdg_toplevel_icons);

#[derive(Debug, Error)]
pub enum XdgToplevelIconV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Toplevel icon is immutable")]
    Immutable,
    #[error("Buffer is not a square")]
    NotSquare,
}
efrom!(XdgToplevelIconV1Error, ClientError);
