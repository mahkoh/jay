use {
    crate::{
        client::{Client, ClientError},
        format::XRGB8888,
        gfx_api::{GfxContext, GfxError, GfxFramebuffer, GfxTexture},
        ifs::jay_output::JayOutput,
        leaks::Tracker,
        object::Object,
        tree::{OutputNode, WorkspaceNodeId},
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            option_ext::OptionExt,
        },
        video::{
            dmabuf::DmaBuf,
            gbm::{GbmError, GBM_BO_USE_LINEAR, GBM_BO_USE_RENDERING},
            Modifier, INVALID_MODIFIER, LINEAR_MODIFIER,
        },
        wire::{jay_screencast::*, JayScreencastId},
    },
    ahash::AHashSet,
    indexmap::{indexset, IndexSet},
    once_cell::sync::Lazy,
    std::{
        cell::{Cell, RefCell},
        ops::{Deref, DerefMut},
        rc::Rc,
    },
    thiserror::Error,
};

pub struct JayScreencast {
    pub id: JayScreencastId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    config_serial: NumCell<u32>,
    config_acked: Cell<bool>,
    buffers_serial: NumCell<u32>,
    buffers_acked: Cell<bool>,
    buffers: RefCell<Vec<ScreencastBuffer>>,
    missed_frame: Cell<bool>,
    output: CloneCell<Option<Rc<OutputNode>>>,
    destroyed: Cell<bool>,
    running: Cell<bool>,
    show_all: Cell<bool>,
    show_workspaces: RefCell<AHashSet<WorkspaceNodeId>>,
    linear: Cell<bool>,
    pending: Pending,
}

#[derive(Default)]
struct Pending {
    linear: Cell<Option<bool>>,
    running: Cell<Option<bool>>,
    output: Cell<Option<Option<Rc<JayOutput>>>>,
    show_all: Cell<Option<bool>>,
    show_workspaces: RefCell<Option<AHashSet<WorkspaceNodeId>>>,
}

struct ScreencastBuffer {
    dmabuf: DmaBuf,
    fb: Rc<dyn GfxFramebuffer>,
    free: bool,
}

impl JayScreencast {
    pub fn new(id: JayScreencastId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            config_serial: Default::default(),
            config_acked: Cell::new(true),
            buffers_serial: Default::default(),
            buffers_acked: Cell::new(false),
            buffers: Default::default(),
            missed_frame: Cell::new(false),
            output: Default::default(),
            destroyed: Cell::new(false),
            running: Cell::new(false),
            show_all: Cell::new(false),
            show_workspaces: Default::default(),
            linear: Cell::new(false),
            pending: Default::default(),
        }
    }

    fn send_buffers(&self) {
        self.buffers_acked.set(false);
        let serial = self.buffers_serial.fetch_add(1) + 1;
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

    #[allow(dead_code)]
    fn send_config(&self) {
        self.config_acked.set(false);
        let serial = self.config_serial.fetch_add(1) + 1;
        if let Some(output) = self.output.get() {
            self.client.event(ConfigOutput {
                self_id: self.id,
                linear_id: output.id.raw(),
            });
        }
        self.client.event(ConfigAllowAllWorkspaces {
            self_id: self.id,
            allow_all: self.show_all.get() as _,
        });
        for &ws in self.show_workspaces.borrow_mut().iter() {
            self.client.event(ConfigAllowWorkspace {
                self_id: self.id,
                linear_id: ws.raw(),
            });
        }
        self.client.event(ConfigUseLinearBuffers {
            self_id: self.id,
            use_linear: self.linear.get() as _,
        });
        self.client.event(ConfigRunning {
            self_id: self.id,
            running: self.running.get() as _,
        });
        self.client.event(ConfigDone {
            self_id: self.id,
            serial,
        });
    }

    pub fn copy_texture(&self, on: &OutputNode, texture: &Rc<dyn GfxTexture>) {
        if !self.running.get() {
            return;
        }
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
                buffer
                    .fb
                    .copy_texture(&self.client.state, texture, 0, 0, false);
                self.client.event(Ready {
                    self_id: self.id,
                    idx: idx as _,
                });
                buffer.free = false;
                return;
            }
        }
        self.missed_frame.set(true);
        self.client.event(MissedFrame { self_id: self.id })
    }

    fn detach(&self) {
        if let Some(output) = self.output.take() {
            output.screencasts.remove(&(self.client.id, self.id));
            if output.screencasts.is_empty() {
                output.state.damage();
            }
        }
    }

    pub fn do_destroy(&self) {
        self.detach();
        self.destroyed.set(true);
        self.client.event(Destroyed { self_id: self.id });
    }

    pub fn realloc(&self, ctx: &Rc<dyn GfxContext>) -> Result<(), JayScreencastError> {
        let mut buffers = vec![];
        let formats = ctx.formats();
        let format = match formats.get(&XRGB8888.drm) {
            Some(f) => f,
            _ => return Err(JayScreencastError::XRGB8888),
        };
        if let Some(output) = self.output.get() {
            let mode = output.global.mode.get();
            let num = 3;
            for _ in 0..num {
                let mut usage = GBM_BO_USE_RENDERING;
                let modifiers = match self.linear.get() {
                    true if format.write_modifiers.contains(&LINEAR_MODIFIER) => {
                        static MODS: Lazy<IndexSet<Modifier>> =
                            Lazy::new(|| indexset![LINEAR_MODIFIER]);
                        &MODS
                    }
                    true if format.write_modifiers.contains(&INVALID_MODIFIER) => {
                        usage |= GBM_BO_USE_LINEAR;
                        static MODS: Lazy<IndexSet<Modifier>> =
                            Lazy::new(|| indexset![INVALID_MODIFIER]);
                        &MODS
                    }
                    true => return Err(JayScreencastError::Modifier),
                    false if format.write_modifiers.is_empty() => {
                        return Err(JayScreencastError::XRGB8888Writing)
                    }
                    false => &format.write_modifiers,
                };
                let buffer =
                    ctx.gbm()
                        .create_bo(mode.width, mode.height, XRGB8888, modifiers, usage)?;
                let fb = ctx.clone().dmabuf_img(buffer.dmabuf())?.to_framebuffer()?;
                buffers.push(ScreencastBuffer {
                    dmabuf: buffer.dmabuf().clone(),
                    fb,
                    free: true,
                });
            }
        }
        *self.buffers.borrow_mut() = buffers;
        self.send_buffers();
        self.damage();
        Ok(())
    }

    fn damage(&self) {
        if let Some(output) = self.output.get() {
            output.global.connector.connector.damage();
        }
    }
}

impl JayScreencast {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_output(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: SetOutput = self.client.parse(self, parser)?;
        let output = if req.output.is_some() {
            Some(self.client.lookup(req.output)?)
        } else {
            None
        };
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.output.set(Some(output));
        Ok(())
    }

    fn set_allow_all_workspaces(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), JayScreencastError> {
        let req: SetAllowAllWorkspaces = self.client.parse(self, parser)?;
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.show_all.set(Some(req.allow_all != 0));
        Ok(())
    }

    fn allow_workspace(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: AllowWorkspace = self.client.parse(self, parser)?;
        let ws = self.client.lookup(req.workspace)?;
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        let mut sw = self.pending.show_workspaces.borrow_mut();
        let sw = sw.get_or_insert_default_ext();
        if let Some(ws) = ws.workspace.get() {
            sw.insert(ws.id);
        }
        Ok(())
    }

    fn touch_allowed_workspaces(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), JayScreencastError> {
        let _req: TouchAllowedWorkspaces = self.client.parse(self, parser)?;
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending
            .show_workspaces
            .borrow_mut()
            .get_or_insert_default_ext();
        Ok(())
    }

    fn set_use_linear_buffers(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: SetUseLinearBuffers = self.client.parse(self, parser)?;
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.linear.set(Some(req.use_linear != 0));
        Ok(())
    }

    fn set_running(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: SetRunning = self.client.parse(self, parser)?;
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.running.set(Some(req.running != 0));
        Ok(())
    }

    fn configure(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let _req: Configure = self.client.parse(self.deref(), parser)?;

        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }

        let mut need_realloc = false;

        if let Some(output) = self.pending.output.take() {
            let output = output.and_then(|o| o.output.get());
            if output_size(&output) != output_size(&self.output.get()) {
                need_realloc = true;
            }
            self.detach();
            if let Some(new) = &output {
                if new.screencasts.is_empty() {
                    new.state.damage();
                }
                new.screencasts.set((self.client.id, self.id), self.clone());
            }
            self.output.set(output);
        }
        if let Some(linear) = self.pending.linear.take() {
            if self.linear.replace(linear) != linear {
                need_realloc = true;
            }
        }
        if let Some(show_all) = self.pending.show_all.take() {
            self.show_all.set(show_all);
        }
        if let Some(new_workspaces) = self.pending.show_workspaces.borrow_mut().take() {
            *self.show_workspaces.borrow_mut() = new_workspaces;
        }
        if let Some(running) = self.pending.running.take() {
            self.running.set(running);
        }

        if need_realloc {
            let ctx = match self.client.state.render_ctx.get() {
                Some(ctx) => ctx,
                _ => {
                    self.do_destroy();
                    return Ok(());
                }
            };
            if let Err(e) = self.realloc(&ctx) {
                log::error!("Could not allocate buffers: {}", ErrorFmt(e));
                self.do_destroy();
                return Ok(());
            }
        }

        Ok(())
    }

    fn ack_buffers(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: AckBuffers = self.client.parse(self, parser)?;
        if self.destroyed.get() {
            return Ok(());
        }
        if req.serial == self.buffers_serial.get() {
            self.buffers_acked.set(true);
        }
        Ok(())
    }

    fn ack_config(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: AckConfig = self.client.parse(self, parser)?;
        if self.destroyed.get() {
            return Ok(());
        }
        if req.serial == self.config_serial.get() {
            self.config_acked.set(true);
        }
        Ok(())
    }

    fn release_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), JayScreencastError> {
        let req: ReleaseBuffer = self.client.parse(self, parser)?;
        if self.destroyed.get() || !self.buffers_acked.get() {
            return Ok(());
        }
        let idx = req.idx as usize;
        if idx > self.buffers.borrow_mut().len() {
            return Err(JayScreencastError::OutOfBounds(req.idx));
        }
        self.buffers.borrow_mut()[idx].free = true;
        if self.missed_frame.replace(false) {
            self.damage();
        }
        Ok(())
    }
}

object_base! {
    self = JayScreencast;

    DESTROY => destroy,
    SET_OUTPUT => set_output,
    SET_ALLOW_ALL_WORKSPACES => set_allow_all_workspaces,
    ALLOW_WORKSPACE => allow_workspace,
    TOUCH_ALLOWED_WORKSPACES => touch_allowed_workspaces,
    SET_USE_LINEAR_BUFFERS => set_use_linear_buffers,
    SET_RUNNING => set_running,
    CONFIGURE => configure,
    ACK_CONFIG => ack_config,
    ACK_BUFFERS => ack_buffers,
    RELEASE_BUFFER => release_buffer,
}

impl Object for JayScreencast {
    fn break_loops(&self) {
        self.detach();
    }
}

dedicated_add_obj!(JayScreencast, JayScreencastId, screencasts);

#[derive(Debug, Error)]
pub enum JayScreencastError {
    #[error("Parsing failed")]
    MsgParserError(Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Buffer index {0} is out-of-bounds")]
    OutOfBounds(u32),
    #[error(transparent)]
    GbmError(#[from] GbmError),
    #[error(transparent)]
    GfxError(#[from] GfxError),
    #[error("Render context does not support XRGB8888 format")]
    XRGB8888,
    #[error("Render context does not support XRGB8888 format for rendering")]
    XRGB8888Writing,
    #[error("Render context supports neither linear or invalid modifier")]
    Modifier,
}
efrom!(JayScreencastError, MsgParserError);
efrom!(JayScreencastError, ClientError);

fn output_size(output: &Option<Rc<OutputNode>>) -> (i32, i32) {
    match output {
        Some(o) => {
            let mode = o.global.mode.get();
            (mode.width, mode.height)
        }
        _ => (0, 0),
    }
}
