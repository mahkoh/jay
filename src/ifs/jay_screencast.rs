use {
    crate::{
        client::{Client, ClientError},
        format::XRGB8888,
        ifs::jay_output::JayOutput,
        leaks::Tracker,
        object::Object,
        render::{Framebuffer, RenderContext, RenderError, Texture},
        tree::{OutputNode, WorkspaceNodeId},
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
            errorfmt::ErrorFmt,
            numcell::NumCell,
        },
        video::{
            dmabuf::DmaBuf,
            gbm::{GbmError, GBM_BO_USE_RENDERING},
            ModifiedFormat, INVALID_MODIFIER,
        },
        wire::{jay_screencast::*, JayScreencastId},
    },
    ahash::AHashSet,
    std::{
        cell::{Cell, RefCell},
        ops::{Deref, DerefMut},
        rc::Rc,
    },
    thiserror::Error,
};
use crate::video::gbm::GBM_BO_USE_LINEAR;

pub struct JayScreencast {
    pub id: JayScreencastId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    serial: NumCell<u32>,
    acked: Cell<bool>,
    buffers: RefCell<Vec<ScreencastBuffer>>,
    missed_frame: Cell<bool>,
    output: CloneCell<Option<Rc<JayOutput>>>,
    started: Cell<bool>,
    destroyed: Cell<bool>,
    show_all: Cell<bool>,
    show_workspaces: RefCell<AHashSet<WorkspaceNodeId>>,
}

struct ScreencastBuffer {
    dmabuf: DmaBuf,
    fb: Rc<Framebuffer>,
    free: bool,
}

impl JayScreencast {
    pub fn new(id: JayScreencastId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            serial: NumCell::new(0),
            acked: Cell::new(false),
            buffers: Default::default(),
            missed_frame: Cell::new(false),
            output: Default::default(),
            started: Cell::new(false),
            destroyed: Cell::new(false),
            show_all: Cell::new(false),
            show_workspaces: Default::default(),
        }
    }

    fn send_buffers(&self) {
        self.acked.set(false);
        let serial = self.serial.fetch_add(1) + 1;
        let buffers = self.buffers.borrow_mut();
        for buffer in buffers.iter() {
            for plane in &buffer.dmabuf.planes {
                self.client.event(Plane {
                    self_id: self.id,
                    fd: plane.fd.clone(),
                    offset: plane.offset,
                    stride: plane.stride,
                });
            }
            self.client.event(Buffer {
                self_id: self.id,
                format: buffer.dmabuf.format.drm,
                modifier: buffer.dmabuf.modifier,
                width: buffer.dmabuf.width,
                height: buffer.dmabuf.height,
            });
        }
        self.client.event(BuffersDone {
            self_id: self.id,
            serial,
        });
    }

    pub fn send_ready(&self, idx: u32) {
        self.client.event(Ready {
            self_id: self.id,
            idx,
        })
    }

    pub fn send_destroyed(&self) {
        self.client.event(Destroyed { self_id: self.id });
    }

    pub fn copy_texture(&self, on: &OutputNode, texture: &Texture) {
        if !self.show_all.get() {
            let ws = match on.workspace.get() {
                Some(ws) => ws,
                _ => return,
            };
            if !self.show_workspaces.borrow_mut().contains(&ws.id) {
                return;
            }
        }
        let mut buffer = self.buffers.borrow_mut();
        for (idx, buffer) in buffer.deref_mut().iter_mut().enumerate() {
            if buffer.free {
                buffer.fb.copy_texture(&self.client.state, texture, 0, 0);
                self.send_ready(idx as _);
                buffer.free = false;
                return;
            }
        }
        self.missed_frame.set(true);
    }

    fn detach(&self) {
        if let Some(output) = self.output.get() {
            if let Some(output) = output.output.get() {
                output.screencasts.remove(&(self.client.id, self.id));
            }
        }
    }

    pub fn do_destroy(&self) {
        self.detach();
        self.destroyed.set(true);
        self.send_destroyed();
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_output(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: SetOutput = self.client.parse(self, parser)?;
        let output = self.client.lookup(req.output)?;
        if self.started.get() {
            return Err(JayScreencastError::AlreadyStarted);
        }
        if self.destroyed.get() {
            return Ok(());
        }
        self.output.set(Some(output));
        Ok(())
    }

    fn set_show_always(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let _req: SetShowAlways = self.client.parse(self, parser)?;
        if self.started.get() {
            return Err(JayScreencastError::AlreadyStarted);
        }
        if self.destroyed.get() {
            return Ok(());
        }
        self.show_all.set(true);
        Ok(())
    }

    fn add_workspace(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: AddWorkspace = self.client.parse(self, parser)?;
        let ws = self.client.lookup(req.workspace)?;
        if self.started.get() {
            return Err(JayScreencastError::AlreadyStarted);
        }
        if self.destroyed.get() {
            return Ok(());
        }
        let ws = match ws.workspace.get() {
            Some(ws) => ws,
            _ => return Ok(()),
        };
        self.show_workspaces.borrow_mut().insert(ws.id);
        Ok(())
    }

    pub fn allocate_buffers(
        &self,
        output: &OutputNode,
        ctx: &Rc<RenderContext>,
    ) -> Result<(), JayScreencastError> {
        let mode = output.global.mode.get();
        let mut buffers = vec![];
        let num = 3;
        for _ in 0..num {
            let format = ModifiedFormat {
                format: XRGB8888,
                modifier: INVALID_MODIFIER,
            };
            let buffer =
                ctx.gbm
                    .create_bo(mode.width, mode.height, &format, GBM_BO_USE_RENDERING)?;
            let fb = ctx.dmabuf_img(buffer.dmabuf())?.to_framebuffer()?;
            buffers.push(ScreencastBuffer {
                dmabuf: buffer.dmabuf().clone(),
                fb,
                free: true,
            });
        }
        *self.buffers.borrow_mut() = buffers;
        self.send_buffers();
        Ok(())
    }

    fn start(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let _req: Start = self.client.parse(self.deref(), parser)?;
        if self.started.replace(true) {
            return Err(JayScreencastError::AlreadyStarted);
        }
        if self.destroyed.get() {
            return Ok(());
        }
        let output = match self.output.get() {
            Some(o) => o,
            _ => return Err(JayScreencastError::NoOutputSet),
        };
        let output = match output.output.get() {
            Some(o) => o,
            _ => {
                self.do_destroy();
                return Ok(());
            }
        };
        let ctx = match self.client.state.render_ctx.get() {
            Some(ctx) => ctx,
            _ => {
                self.do_destroy();
                return Ok(());
            }
        };
        if let Err(e) = self.allocate_buffers(&output, &ctx) {
            log::error!("Could not allocate buffer: {}", ErrorFmt(e));
            self.do_destroy();
            return Ok(());
        }
        output
            .screencasts
            .set((self.client.id, self.id), self.clone());
        output.global.connector.connector.damage();
        Ok(())
    }

    fn ack(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: Ack = self.client.parse(self, parser)?;
        if self.destroyed.get() {
            return Ok(());
        }
        if req.serial == self.serial.get() {
            self.acked.set(true);
        }
        Ok(())
    }

    fn release_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: ReleaseBuffer = self.client.parse(self, parser)?;
        if self.destroyed.get() {
            return Ok(());
        }
        if self.acked.get() {
            let idx = req.idx as usize;
            if idx > self.buffers.borrow_mut().len() {
                return Err(JayScreencastError::OutOfBounds(req.idx));
            }
            self.buffers.borrow_mut()[idx].free = true;
            if self.missed_frame.replace(false) {
                if let Some(output) = self.output.get() {
                    if let Some(output) = output.output.get() {
                        output.global.connector.connector.damage();
                    }
                }
            }
        }
        Ok(())
    }
}

object_base! {
    JayScreencast;

    DESTROY => destroy,
    SET_OUTPUT => set_output,
    SET_SHOW_ALWAYS => set_show_always,
    ADD_WORKSPACE => add_workspace,
    START => start,
    ACK => ack,
    RELEASE_BUFFER => release_buffer,
}

impl Object for JayScreencast {
    fn num_requests(&self) -> u32 {
        RELEASE_BUFFER + 1
    }

    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(JayScreencast);

#[derive(Debug, Error)]
pub enum JayScreencastError {
    #[error("Parsing failed")]
    MsgParserError(Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Buffer index {0} is out-of-bounds")]
    OutOfBounds(u32),
    #[error("The screencast has already been started")]
    AlreadyStarted,
    #[error("No output has been set")]
    NoOutputSet,
    #[error(transparent)]
    GbmError(#[from] GbmError),
    #[error(transparent)]
    RenderError(#[from] RenderError),
}
efrom!(JayScreencastError, MsgParserError);
efrom!(JayScreencastError, ClientError);
