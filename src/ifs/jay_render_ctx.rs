use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::Object,
        render::RenderContext,
        utils::{
            buffd::{MsgParser, MsgParserError},
            errorfmt::ErrorFmt,
        },
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
    pub fn send_render_ctx(&self, ctx: Option<&Rc<RenderContext>>) {
        let mut fd = None;
        if let Some(ctx) = ctx {
            match ctx.gbm.drm.dup_render() {
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), JayRenderCtxError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.remove_from_state();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn remove_from_state(&self) {
        self.client
            .state
            .render_ctx_watchers
            .remove(&(self.client.id, self.id));
    }
}

object_base! {
    JayRenderCtx;

    DESTROY => destroy,
}

impl Object for JayRenderCtx {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.remove_from_state();
    }
}

simple_add_obj!(JayRenderCtx);

#[derive(Debug, Error)]
pub enum JayRenderCtxError {
    #[error("Parsing failed")]
    MsgParserError(Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayRenderCtxError, MsgParserError);
efrom!(JayRenderCtxError, ClientError);
