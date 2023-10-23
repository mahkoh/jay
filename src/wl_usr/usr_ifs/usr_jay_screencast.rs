use {
    crate::{
        format::formats,
        ifs::jay_workspace::JayWorkspace,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        video::dmabuf::{DmaBuf, DmaBufPlane, PlaneVec},
        wire::{jay_screencast::*, JayScreencastId},
        wl_usr::{usr_ifs::usr_jay_output::UsrJayOutput, usr_object::UsrObject, UsrCon},
    },
    std::{cell::RefCell, mem, ops::DerefMut, rc::Rc},
    thiserror::Error,
};

pub struct UsrJayScreencast {
    pub id: JayScreencastId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayScreencastOwner>>>,

    pub pending_buffers: RefCell<PlaneVec<DmaBuf>>,
    pub pending_planes: RefCell<PlaneVec<DmaBufPlane>>,

    pub pending_config: RefCell<UsrJayScreencastServerConfig>,
}

#[derive(Default)]
pub struct UsrJayScreencastServerConfig {
    pub output: Option<u32>,
    pub show_all: bool,
    pub running: bool,
    pub use_linear_buffers: bool,
    pub allowed_workspaces: Vec<u32>,
}

pub trait UsrJayScreencastOwner {
    fn buffers(&self, buffers: PlaneVec<DmaBuf>) {
        let _ = buffers;
    }

    fn ready(&self, ev: &Ready) {
        let _ = ev;
    }

    fn destroyed(&self) {}

    fn missed_frame(&self) {}

    fn config(&self, config: UsrJayScreencastServerConfig) {
        let _ = config;
    }
}

impl UsrJayScreencast {
    pub fn set_output(&self, output: &UsrJayOutput) {
        self.con.request(SetOutput {
            self_id: self.id,
            output: output.id,
        });
    }

    pub fn set_allow_all_workspaces(&self, allow_all: bool) {
        self.con.request(SetAllowAllWorkspaces {
            self_id: self.id,
            allow_all: allow_all as _,
        });
    }

    #[allow(dead_code)]
    pub fn allow_workspace(&self, ws: &JayWorkspace) {
        self.con.request(AllowWorkspace {
            self_id: self.id,
            workspace: ws.id,
        });
    }

    #[allow(dead_code)]
    pub fn touch_allowed_workspaces(&self) {
        self.con
            .request(TouchAllowedWorkspaces { self_id: self.id });
    }

    pub fn set_use_linear_buffers(&self, linear: bool) {
        self.con.request(SetUseLinearBuffers {
            self_id: self.id,
            use_linear: linear as _,
        });
    }

    pub fn set_running(&self, running: bool) {
        self.con.request(SetRunning {
            self_id: self.id,
            running: running as _,
        });
    }

    pub fn configure(&self) {
        self.con.request(Configure { self_id: self.id });
    }

    pub fn release_buffer(&self, idx: usize) {
        self.con.request(ReleaseBuffer {
            self_id: self.id,
            idx: idx as _,
        });
    }

    fn plane(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Plane = self.con.parse(self, parser)?;
        self.pending_planes.borrow_mut().push(DmaBufPlane {
            offset: ev.offset,
            stride: ev.stride,
            fd: ev.fd,
        });
        Ok(())
    }

    fn buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), UsrJayScreencastError> {
        let ev: Buffer = self.con.parse(self, parser)?;
        let format = match formats().get(&ev.format) {
            Some(f) => f,
            _ => return Err(UsrJayScreencastError::UnknownFormat(ev.format)),
        };
        self.pending_buffers.borrow_mut().push(DmaBuf {
            width: ev.width,
            height: ev.height,
            format,
            modifier: ev.modifier,
            planes: mem::take(self.pending_planes.borrow_mut().deref_mut()),
        });
        Ok(())
    }

    fn buffers_done(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: BuffersDone = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.buffers(mem::take(self.pending_buffers.borrow_mut().deref_mut()));
        }
        self.con.request(AckBuffers {
            self_id: self.id,
            serial: ev.serial,
        });
        Ok(())
    }

    fn ready(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Ready = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.ready(&ev);
        }
        Ok(())
    }

    fn destroyed(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Destroyed = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }

    fn missed_frame(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: MissedFrame = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.missed_frame();
        }
        Ok(())
    }

    fn config_output(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: ConfigOutput = self.con.parse(self, parser)?;
        self.pending_config.borrow_mut().output = Some(ev.linear_id);
        Ok(())
    }

    fn config_allow_all_workspaces(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: ConfigAllowAllWorkspaces = self.con.parse(self, parser)?;
        self.pending_config.borrow_mut().show_all = ev.allow_all != 0;
        Ok(())
    }

    fn config_use_linear_buffers(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: ConfigUseLinearBuffers = self.con.parse(self, parser)?;
        self.pending_config.borrow_mut().use_linear_buffers = ev.use_linear != 0;
        Ok(())
    }

    fn config_running(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: ConfigRunning = self.con.parse(self, parser)?;
        self.pending_config.borrow_mut().running = ev.running != 0;
        Ok(())
    }

    fn config_allow_workspace(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: ConfigAllowWorkspace = self.con.parse(self, parser)?;
        self.pending_config
            .borrow_mut()
            .allowed_workspaces
            .push(ev.linear_id);
        Ok(())
    }

    fn config_done(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: ConfigDone = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.config(mem::take(self.pending_config.borrow_mut().deref_mut()));
        }
        self.con.request(AckConfig {
            self_id: self.id,
            serial: ev.serial,
        });
        Ok(())
    }
}

usr_object_base! {
    UsrJayScreencast, JayScreencast;

    PLANE => plane,
    BUFFER => buffer,
    BUFFERS_DONE => buffers_done,
    READY => ready,
    DESTROYED => destroyed,
    MISSED_FRAME => missed_frame,
    CONFIG_OUTPUT => config_output,
    CONFIG_ALLOW_ALL_WORKSPACES => config_allow_all_workspaces,
    CONFIG_ALLOW_WORKSPACE => config_allow_workspace,
    CONFIG_USE_LINEAR_BUFFERS => config_use_linear_buffers,
    CONFIG_RUNNING => config_running,
    CONFIG_DONE => config_done,
}

impl UsrObject for UsrJayScreencast {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}

#[derive(Debug, Error)]
pub enum UsrJayScreencastError {
    #[error("Parsing failed")]
    MsgParserError(#[from] MsgParserError),
    #[error("The server sent an unknown format {0}")]
    UnknownFormat(u32),
}
