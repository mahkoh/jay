use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
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
}

impl ZwpIdleInhibitorV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpIdleInhibitorV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        if self.surface.idle_inhibitors.remove(&self.id).is_some() {
            self.deactivate();
        }
        Ok(())
    }

    pub fn install(self: &Rc<Self>) -> Result<(), ZwpIdleInhibitorV1Error> {
        self.surface.idle_inhibitors.set(self.id, self.clone());
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

object_base2! {
    ZwpIdleInhibitorV1;

    DESTROY => destroy,
}

impl Object for ZwpIdleInhibitorV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.deactivate();
    }
}

simple_add_obj!(ZwpIdleInhibitorV1);

#[derive(Debug, Error)]
pub enum ZwpIdleInhibitorV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpIdleInhibitorV1Error, ClientError);
efrom!(ZwpIdleInhibitorV1Error, MsgParserError);
