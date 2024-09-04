use {
    crate::{
        format::formats,
        object::Version,
        utils::clonecell::CloneCell,
        video::dmabuf::{DmaBuf, DmaBufPlane, PlaneVec},
        wire::{jay_screencast::*, JayScreencastId},
        wl_usr::{
            usr_ifs::{
                usr_jay_output::UsrJayOutput, usr_jay_toplevel::UsrJayToplevel,
                usr_jay_workspace::UsrJayWorkspace, usr_wl_buffer::UsrWlBuffer,
            },
            usr_object::UsrObject,
            UsrCon,
        },
    },
    std::{cell::RefCell, mem, ops::DerefMut, rc::Rc},
    thiserror::Error,
};

pub struct UsrJayScreencast {
    pub id: JayScreencastId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayScreencastOwner>>>,
    pub version: Version,

    pub pending_buffers: RefCell<Vec<DmaBuf>>,
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
    pub width: i32,
    pub height: i32,
}

pub trait UsrJayScreencastOwner {
    fn buffers(&self, buffers: Vec<DmaBuf>) {
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

    pub fn set_toplevel(&self, tl: &UsrJayToplevel) {
        self.con.request(SetToplevel {
            self_id: self.id,
            id: tl.id,
        });
    }

    pub fn set_allow_all_workspaces(&self, allow_all: bool) {
        self.con.request(SetAllowAllWorkspaces {
            self_id: self.id,
            allow_all: allow_all as _,
        });
    }

    pub fn allow_workspace(&self, ws: &UsrJayWorkspace) {
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

    pub fn clear_buffers(&self) {
        self.con.request(ClearBuffers { self_id: self.id });
    }

    pub fn add_buffer(&self, buffer: &UsrWlBuffer) {
        self.con.request(AddBuffer {
            self_id: self.id,
            buffer: buffer.id,
        });
    }
}

impl JayScreencastEventHandler for UsrJayScreencast {
    type Error = UsrJayScreencastError;

    fn plane(&self, ev: Plane, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending_planes.borrow_mut().push(DmaBufPlane {
            offset: ev.offset,
            stride: ev.stride,
            fd: ev.fd,
        });
        Ok(())
    }

    fn buffer(&self, ev: Buffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let format = match formats().get(&ev.format) {
            Some(f) => f,
            _ => return Err(UsrJayScreencastError::UnknownFormat(ev.format)),
        };
        self.pending_buffers.borrow_mut().push(DmaBuf {
            id: self.con.dma_buf_ids.next(),
            width: ev.width,
            height: ev.height,
            format,
            modifier: ev.modifier,
            planes: mem::take(self.pending_planes.borrow_mut().deref_mut()),
        });
        Ok(())
    }

    fn buffers_done(&self, ev: BuffersDone, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.buffers(mem::take(self.pending_buffers.borrow_mut().deref_mut()));
        }
        self.con.request(AckBuffers {
            self_id: self.id,
            serial: ev.serial,
        });
        Ok(())
    }

    fn ready(&self, ev: Ready, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.ready(&ev);
        }
        Ok(())
    }

    fn destroyed(&self, _ev: Destroyed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }

    fn missed_frame(&self, _ev: MissedFrame, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.missed_frame();
        }
        Ok(())
    }

    fn config_output(&self, ev: ConfigOutput, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending_config.borrow_mut().output = Some(ev.linear_id);
        Ok(())
    }

    fn config_allow_all_workspaces(
        &self,
        ev: ConfigAllowAllWorkspaces,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.pending_config.borrow_mut().show_all = ev.allow_all != 0;
        Ok(())
    }

    fn config_allow_workspace(
        &self,
        ev: ConfigAllowWorkspace,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.pending_config
            .borrow_mut()
            .allowed_workspaces
            .push(ev.linear_id);
        Ok(())
    }

    fn config_use_linear_buffers(
        &self,
        ev: ConfigUseLinearBuffers,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.pending_config.borrow_mut().use_linear_buffers = ev.use_linear != 0;
        Ok(())
    }

    fn config_running(&self, ev: ConfigRunning, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending_config.borrow_mut().running = ev.running != 0;
        Ok(())
    }

    fn config_done(&self, ev: ConfigDone, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.config(mem::take(self.pending_config.borrow_mut().deref_mut()));
        }
        self.con.request(AckConfig {
            self_id: self.id,
            serial: ev.serial,
        });
        Ok(())
    }

    fn config_size(&self, ev: ConfigSize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending_config.borrow_mut().width = ev.width;
        self.pending_config.borrow_mut().height = ev.height;
        Ok(())
    }
}

usr_object_base! {
    self = UsrJayScreencast = JayScreencast;
    version = self.version;
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
    #[error("The server sent an unknown format {0}")]
    UnknownFormat(u32),
}
