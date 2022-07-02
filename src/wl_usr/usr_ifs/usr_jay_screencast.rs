use {
    crate::{
        format::formats,
        ifs::jay_workspace::JayWorkspace,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        video::dmabuf::{DmaBuf, DmaBufPlane},
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

    pub pending_buffers: RefCell<Vec<DmaBuf>>,
    pub pending_planes: RefCell<Vec<DmaBufPlane>>,
}

pub trait UsrJayScreencastOwner {
    fn buffers(&self, buffers: Vec<DmaBuf>) {
        let _ = buffers;
    }

    fn ready(&self, ev: &Ready) {
        let _ = ev;
    }

    fn destroyed(&self) {}
}

impl UsrJayScreencast {
    pub fn request_set_output(&self, output: &UsrJayOutput) {
        self.con.request(SetOutput {
            self_id: self.id,
            output: output.id,
        });
    }

    pub fn request_set_show_always(&self) {
        self.con.request(SetShowAlways { self_id: self.id });
    }

    pub fn request_add_workspace(&self, ws: &JayWorkspace) {
        self.con.request(AddWorkspace {
            self_id: self.id,
            workspace: ws.id,
        });
    }

    pub fn request_start(&self) {
        self.con.request(Start { self_id: self.id });
    }

    fn request_ack(&self, serial: u32) {
        self.con.request(Ack {
            self_id: self.id,
            serial,
        });
    }

    pub fn request_release_buffer(&self, idx: usize) {
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
        self.request_ack(ev.serial);
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
}

impl Drop for UsrJayScreencast {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrJayScreencast, JayScreencast;

    PLANE => plane,
    BUFFER => buffer,
    BUFFERS_DONE => buffers_done,
    READY => ready,
    DESTROYED => destroyed,
}

impl UsrObject for UsrJayScreencast {
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
