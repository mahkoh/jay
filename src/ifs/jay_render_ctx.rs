use {
    crate::{
        client::{Client, ClientError},
        gfx_api::GfxContext,
        leaks::Tracker,
        object::{Object, Version},
        utils::errorfmt::ErrorFmt,
        wire::{jay_render_ctx::*, JayRenderCtxId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayRenderCtx {
    pub id: JayRenderCtxId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JayRenderCtx {
    pub fn send_render_ctx(&self, ctx: Option<Rc<dyn GfxContext>>) {
        let mut fd = None;
        if let Some(ctx) = ctx {
            match ctx.gbm().drm.dup_render() {
                Ok(d) => fd = Some(d.fd().clone()),
                Err(e) => {
                    log::error!("Could not dup drm fd: {}", ErrorFmt(e));
                }
            }
        } else {
            self.client.event(NoDevice { self_id: self.id });
        }
        match fd {
            Some(fd) => self.client.event(Device {
                self_id: self.id,
                fd,
            }),
            _ => self.client.event(NoDevice { self_id: self.id }),
        }
    }

    fn remove_from_state(&self) {
        self.client
            .state
            .render_ctx_watchers
            .remove(&(self.client.id, self.id));
    }
}

impl JayRenderCtxRequestHandler for JayRenderCtx {
    type Error = JayRenderCtxError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.remove_from_state();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayRenderCtx;
    version = Version(1);
}

impl Object for JayRenderCtx {
    fn break_loops(&self) {
        self.remove_from_state();
    }
}

simple_add_obj!(JayRenderCtx);

#[derive(Debug, Error)]
pub enum JayRenderCtxError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayRenderCtxError, ClientError);
