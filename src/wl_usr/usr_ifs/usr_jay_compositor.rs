use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{jay_compositor::*, JayCompositorId},
        wl_usr::{
            usr_ifs::{
                usr_jay_output::UsrJayOutput, usr_jay_render_ctx::UsrJayRenderCtx,
                usr_jay_screencast::UsrJayScreencast, usr_wl_output::UsrWlOutput,
            },
            usr_object::UsrObject,
            UsrCon,
        },
    },
    std::rc::Rc,
};

pub struct UsrJayCompositor {
    pub id: JayCompositorId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayCompositorOwner>>>,
}

pub trait UsrJayCompositorOwner {
    fn client_id(&self, ev: ClientId) {
        let _ = ev;
    }

    fn seat(&self, ev: Seat) {
        let _ = ev;
    }
}

impl UsrJayCompositor {
    pub fn request_get_render_context(&self, jo: &UsrJayRenderCtx) {
        self.con.request(GetRenderCtx {
            self_id: self.id,
            id: jo.id,
        });
    }

    pub fn request_create_screencast(&self, sc: &UsrJayScreencast) {
        self.con.request(CreateScreencast {
            self_id: self.id,
            id: sc.id,
        });
    }

    pub fn request_get_output(&self, jo: &UsrJayOutput, output: &UsrWlOutput) {
        self.con.request(GetOutput {
            self_id: self.id,
            id: jo.id,
            output: output.id,
        });
    }

    pub fn request_watch_workspaces(&self, jo: &UsrJayOutput, output: &UsrWlOutput) {
        self.con.request(GetOutput {
            self_id: self.id,
            id: jo.id,
            output: output.id,
        });
    }

    fn client_id(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: ClientId = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.client_id(ev);
        }
        Ok(())
    }

    fn seat(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Seat = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.seat(ev);
        }
        Ok(())
    }
}

impl Drop for UsrJayCompositor {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrJayCompositor, JayCompositor;

    CLIENT_ID => client_id,
    SEAT => seat,
}

impl UsrObject for UsrJayCompositor {
    fn break_loops(&self) {
        self.owner.take();
    }
}
