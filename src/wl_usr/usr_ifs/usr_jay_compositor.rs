use {
    crate::{
        ifs::jay_compositor::Cap,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{jay_compositor::*, JayCompositorId},
        wl_usr::{
            usr_ifs::{
                usr_jay_output::UsrJayOutput, usr_jay_pointer::UsrJayPointer,
                usr_jay_render_ctx::UsrJayRenderCtx, usr_jay_screencast::UsrJayScreencast,
                usr_jay_workspace_watcher::UsrJayWorkspaceWatcher, usr_wl_output::UsrWlOutput,
                usr_wl_seat::UsrWlSeat,
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
    pub fn get_render_context(&self) -> Rc<UsrJayRenderCtx> {
        let rc = Rc::new(UsrJayRenderCtx {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
        });
        self.con.request(GetRenderCtx {
            self_id: self.id,
            id: rc.id,
        });
        self.con.add_object(rc.clone());
        rc
    }

    pub fn create_screencast(&self) -> Rc<UsrJayScreencast> {
        let sc = Rc::new(UsrJayScreencast {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            pending_buffers: Default::default(),
            pending_planes: Default::default(),
            pending_config: Default::default(),
        });
        self.con.request(CreateScreencast {
            self_id: self.id,
            id: sc.id,
        });
        self.con.add_object(sc.clone());
        sc
    }

    pub fn get_output(&self, output: &UsrWlOutput) -> Rc<UsrJayOutput> {
        let jo = Rc::new(UsrJayOutput {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
        });
        self.con.request(GetOutput {
            self_id: self.id,
            id: jo.id,
            output: output.id,
        });
        self.con.add_object(jo.clone());
        jo
    }

    #[allow(dead_code)]
    pub fn watch_workspaces(&self) -> Rc<UsrJayWorkspaceWatcher> {
        let ww = Rc::new(UsrJayWorkspaceWatcher {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
        });
        self.con.request(WatchWorkspaces {
            self_id: self.id,
            id: ww.id,
        });
        self.con.add_object(ww.clone());
        ww
    }

    pub fn get_pointer(&self, seat: &UsrWlSeat) -> Rc<UsrJayPointer> {
        let jp = Rc::new(UsrJayPointer {
            id: self.con.id(),
            con: self.con.clone(),
        });
        self.con.add_object(jp.clone());
        self.con.request(GetPointer {
            self_id: self.id,
            id: jp.id,
            seat: seat.id,
        });
        jp
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

    fn capabilities(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Capabilities = self.con.parse(self, parser)?;
        for &cap in ev.cap {
            match cap {
                Cap::NONE => {}
                _ => {}
            }
        }
        Ok(())
    }
}

usr_object_base! {
    UsrJayCompositor, JayCompositor;

    CLIENT_ID => client_id,
    SEAT => seat,
    CAPABILITIES => capabilities,
}

impl UsrObject for UsrJayCompositor {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
