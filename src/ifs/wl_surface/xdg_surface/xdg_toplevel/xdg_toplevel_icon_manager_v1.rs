use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1::XdgToplevelIconV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{XdgToplevelIconManagerV1Id, xdg_toplevel_icon_manager_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct XdgToplevelIconManagerV1Global {
    name: GlobalName,
}

impl XdgToplevelIconManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgToplevelIconManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), XdgToplevelIconManagerV1Error> {
        let obj = Rc::new(XdgToplevelIconManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            last_size: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        obj.send_sizes();
        Ok(())
    }
}

global_base!(
    XdgToplevelIconManagerV1Global,
    XdgToplevelIconManagerV1,
    XdgToplevelIconManagerV1Error
);

impl Global for XdgToplevelIconManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(XdgToplevelIconManagerV1Global);

pub struct XdgToplevelIconManagerV1 {
    pub id: XdgToplevelIconManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    last_size: Cell<Option<i32>>,
}

impl XdgToplevelIconManagerV1 {
    pub fn send_sizes(&self) {
        let size = self.client.state.theme.title_icon_size();
        if self.last_size.replace(Some(size)) == Some(size) {
            return;
        }
        self.send_icon_size(size);
        self.send_done();
    }

    fn send_icon_size(&self, size: i32) {
        self.client.event(IconSize {
            self_id: self.id,
            size,
        });
    }

    fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }
}

impl XdgToplevelIconManagerV1RequestHandler for XdgToplevelIconManagerV1 {
    type Error = XdgToplevelIconManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn create_icon(&self, req: CreateIcon, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(XdgToplevelIconV1::new(req.id, &self.client, self.version));
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        self.client
            .state
            .toplevel_icons
            .set(obj.toplevel_icon_id, Rc::downgrade(&obj));
        Ok(())
    }

    fn set_icon(&self, req: SetIcon, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let (id, new) = if req.icon.is_none() {
            (None, None)
        } else {
            let icon = self.client.lookup(req.icon)?;
            let was_mutable = !icon.immutable.replace(true);
            if icon.is_empty() {
                (None, None)
            } else {
                if was_mutable {
                    icon.update_sizes();
                }
                (Some(icon.toplevel_icon_id), Some(icon))
            }
        };
        let tl = self.client.lookup(req.toplevel)?;
        if tl.icon.id() == id {
            return Ok(());
        }
        let old = tl.icon.set(new.clone());
        if let Some(i) = old {
            i.toplevels.remove(&tl.id);
        }
        if let Some(i) = &new {
            i.toplevels.set(tl.id, tl.clone());
            if i.has_no_pending() {
                tl.icon_changed();
            }
        } else {
            tl.icon_changed();
        }
        Ok(())
    }
}

object_base! {
    self = XdgToplevelIconManagerV1;
    version = self.version;
}

impl Object for XdgToplevelIconManagerV1 {}

dedicated_add_obj!(
    XdgToplevelIconManagerV1,
    XdgToplevelIconManagerV1Id,
    xdg_toplevel_icon_managers
);

#[derive(Debug, Error)]
pub enum XdgToplevelIconManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(XdgToplevelIconManagerV1Error, ClientError);
