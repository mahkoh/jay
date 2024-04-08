use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_idle_inhibitor_v1::*, ZwpIdleInhibitorV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

linear_ids!(IdleInhibitorIds, IdleInhibitorId, u64);

pub struct ZwpIdleInhibitorV1 {
    pub id: ZwpIdleInhibitorV1Id,
    pub inhibit_id: IdleInhibitorId,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpIdleInhibitorV1RequestHandler for ZwpIdleInhibitorV1 {
    type Error = ZwpIdleInhibitorV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        if self.surface.idle_inhibitors.remove(&self.id).is_some() {
            self.deactivate();
        }
        Ok(())
    }
}

impl ZwpIdleInhibitorV1 {
    pub fn install(self: &Rc<Self>) -> Result<(), ZwpIdleInhibitorV1Error> {
        self.surface.idle_inhibitors.insert(self.id, self.clone());
        if self.surface.visible.get() {
            self.activate();
        }
        Ok(())
    }

    pub fn activate(self: &Rc<Self>) {
        self.client.state.idle.add_inhibitor(self);
    }

    pub fn deactivate(&self) {
        self.client.state.idle.remove_inhibitor(self);
    }
}

object_base! {
    self = ZwpIdleInhibitorV1;
    version = self.version;
}

impl Object for ZwpIdleInhibitorV1 {
    fn break_loops(&self) {
        self.deactivate();
    }
}

simple_add_obj!(ZwpIdleInhibitorV1);

#[derive(Debug, Error)]
pub enum ZwpIdleInhibitorV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpIdleInhibitorV1Error, ClientError);
