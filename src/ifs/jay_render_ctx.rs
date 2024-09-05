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

pub const FORMATS_SINCE: Version = Version(7);
pub const WRITE_MODIFIER_2_SINCE: Version = Version(9);

pub struct JayRenderCtx {
    pub id: JayRenderCtxId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayRenderCtx {
    pub fn send_render_ctx(&self, ctx: Option<Rc<dyn GfxContext>>) {
        let mut fd = None;
        if let Some(ctx) = ctx {
            if self.version >= FORMATS_SINCE {
                for format in ctx.formats().values() {
                    self.client.event(Format {
                        self_id: self.id,
                        format: format.format.drm,
                    });
                    for (modifier, gwm) in &format.write_modifiers {
                        if self.version >= WRITE_MODIFIER_2_SINCE {
                            self.client.event(WriteModifier2 {
                                self_id: self.id,
                                format: format.format.drm,
                                modifier: *modifier,
                                needs_render_usage: gwm.needs_render_usage as _,
                            });
                        } else {
                            self.client.event(WriteModifier {
                                self_id: self.id,
                                format: format.format.drm,
                                modifier: *modifier,
                            });
                        }
                    }
                    for modifier in &format.read_modifiers {
                        self.client.event(ReadModifier {
                            self_id: self.id,
                            format: format.format.drm,
                            modifier: *modifier,
                        });
                    }
                }
            }
            let allocator = ctx.allocator();
            match allocator.drm() {
                Some(drm) => match drm.dup_render() {
                    Ok(d) => fd = Some(d.fd().clone()),
                    Err(e) => {
                        log::error!("Could not dup drm fd: {}", ErrorFmt(e));
                    }
                },
                None => {
                    log::error!("Allocator does not have a DRM device");
                }
            }
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
