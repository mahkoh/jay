use {
    crate::{
        allocator::{AllocatorError, BufferObject, BO_USE_LINEAR, BO_USE_RENDERING},
        client::{Client, ClientError},
        format::XRGB8888,
        gfx_api::{GfxContext, GfxError, GfxFramebuffer, GfxTexture},
        ifs::{jay_output::JayOutput, jay_toplevel::JayToplevel},
        leaks::Tracker,
        object::{Object, Version},
        scale::Scale,
        state::State,
        tree::{LatchListener, OutputNode, ToplevelNode, WorkspaceNode, WorkspaceNodeId},
        utils::{
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            errorfmt::ErrorFmt,
            event_listener::EventListener,
            numcell::NumCell,
            option_ext::OptionExt,
        },
        video::{dmabuf::DmaBuf, INVALID_MODIFIER, LINEAR_MODIFIER},
        wire::{jay_screencast::*, JayScreencastId},
    },
    ahash::AHashSet,
    jay_config::video::Transform,
    std::{
        cell::{Cell, RefCell},
        ops::DerefMut,
        rc::{Rc, Weak},
    },
    thiserror::Error,
};

pub async fn perform_toplevel_screencasts(state: Rc<State>) {
    loop {
        let screencast = state.pending_toplevel_screencasts.pop().await;
        screencast.perform_toplevel_screencast();
    }
}

pub async fn perform_screencast_realloc(state: Rc<State>) {
    loop {
        let screencast = state.pending_toplevel_screencast_reallocs.pop().await;
        screencast.realloc_scheduled.set(false);
        match state.render_ctx.get() {
            None => screencast.do_destroy(),
            Some(ctx) => {
                if let Err(e) = screencast.realloc(&ctx) {
                    screencast.client.error(e);
                }
            }
        }
    }
}

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
    target: CloneCell<Option<Target>>,
    destroyed: Cell<bool>,
    running: Cell<bool>,
    show_all: Cell<bool>,
    show_workspaces: RefCell<AHashSet<WorkspaceNodeId>>,
    linear: Cell<bool>,
    pending: Pending,
    need_realloc: Cell<bool>,
    realloc_scheduled: Cell<bool>,
    latch_listener: EventListener<dyn LatchListener>,
}

#[derive(Clone)]
enum Target {
    Output(Rc<OutputNode>),
    Toplevel(Rc<dyn ToplevelNode>),
}

impl LatchListener for JayScreencast {
    fn after_latch(self: Rc<Self>) {
        self.schedule_toplevel_screencast();
    }
}

unsafe impl UnsafeCellCloneSafe for Target {}

enum PendingTarget {
    Output(Rc<JayOutput>),
    Toplevel(Rc<JayToplevel>),
}

#[derive(Default)]
struct Pending {
    linear: Cell<Option<bool>>,
    running: Cell<Option<bool>>,
    target: Cell<Option<Option<PendingTarget>>>,
    show_all: Cell<Option<bool>>,
    show_workspaces: RefCell<Option<AHashSet<WorkspaceNodeId>>>,
}

struct ScreencastBuffer {
    _bo: Rc<dyn BufferObject>,
    dmabuf: DmaBuf,
    fb: Rc<dyn GfxFramebuffer>,
    free: bool,
}

impl JayScreencast {
    pub fn shows_ws(&self, ws: &WorkspaceNode) -> bool {
        if self.show_all.get() {
            return true;
        }
        for &id in &*self.show_workspaces.borrow() {
            if id == ws.id {
                return true;
            }
        }
        false
    }

    pub fn new(id: JayScreencastId, client: &Rc<Client>, slf: &Weak<Self>) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            config_serial: Default::default(),
            config_acked: Cell::new(true),
            buffers_serial: Default::default(),
            buffers_acked: Cell::new(true),
            buffers: Default::default(),
            missed_frame: Cell::new(false),
            target: Default::default(),
            destroyed: Cell::new(false),
            running: Cell::new(false),
            show_all: Cell::new(false),
            show_workspaces: Default::default(),
            linear: Cell::new(false),
            pending: Default::default(),
            need_realloc: Cell::new(false),
            realloc_scheduled: Cell::new(false),
            latch_listener: EventListener::new(slf.clone()),
        }
    }

    fn schedule_toplevel_screencast(self: &Rc<Self>) {
        if !self.running.get() {
            return;
        }
        self.client
            .state
            .pending_toplevel_screencasts
            .push(self.clone());
    }

    fn perform_toplevel_screencast(&self) {
        if self.destroyed.get() || !self.running.get() {
            return;
        }
        let Some(target) = self.target.get() else {
            return;
        };
        let Target::Toplevel(tl) = target else {
            log::warn!("Tried to perform window screencast for output screencast");
            return;
        };
        let scale = match tl.tl_data().workspace.get() {
            None => Scale::default(),
            Some(w) => w.output.get().global.persistent.scale.get(),
        };
        let mut buffer = self.buffers.borrow_mut();
        for (idx, buffer) in buffer.deref_mut().iter_mut().enumerate() {
            if buffer.free {
                let res = buffer.fb.render_node(
                    tl.tl_as_node(),
                    &self.client.state,
                    Some(tl.node_absolute_position()),
                    None,
                    scale,
                    true,
                    true,
                    false,
                    Transform::None,
                );
                match res {
                    Ok(_) => {
                        self.client.event(Ready {
                            self_id: self.id,
                            idx: idx as _,
                        });
                        buffer.free = false;
                        return;
                    }
                    Err(e) => {
                        log::error!("Could not perform window copy: {}", ErrorFmt(e));
                        break;
                    }
                }
            }
        }
        self.missed_frame.set(true);
        self.client.event(MissedFrame { self_id: self.id })
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
        if let Some(target) = self.target.get() {
            if let Target::Output(output) = target {
                self.client.event(ConfigOutput {
                    self_id: self.id,
                    linear_id: output.id.raw(),
                });
            }
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

    pub fn copy_texture(
        &self,
        on: &OutputNode,
        texture: &Rc<dyn GfxTexture>,
        render_hardware_cursors: bool,
        x_off: i32,
        y_off: i32,
        size: Option<(i32, i32)>,
    ) {
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
                let res = self.client.state.perform_screencopy(
                    texture,
                    &buffer.fb,
                    on.global.pos.get(),
                    render_hardware_cursors,
                    x_off,
                    y_off,
                    size,
                    on.global.persistent.transform.get(),
                );
                match res {
                    Ok(_) => {
                        self.client.event(Ready {
                            self_id: self.id,
                            idx: idx as _,
                        });
                        buffer.free = false;
                        return;
                    }
                    Err(e) => {
                        log::error!("Could not perform screencopy: {}", ErrorFmt(e));
                        break;
                    }
                }
            }
        }
        self.missed_frame.set(true);
        self.client.event(MissedFrame { self_id: self.id })
    }

    fn detach(&self) {
        self.latch_listener.detach();
        if let Some(target) = self.target.take() {
            match target {
                Target::Output(output) => {
                    output.remove_screencast(self);
                }
                Target::Toplevel(tl) => {
                    let data = tl.tl_data();
                    data.jay_screencasts.remove(&(self.client.id, self.id));
                }
            }
        }
    }

    pub fn do_destroy(&self) {
        self.detach();
        self.destroyed.set(true);
        self.client.event(Destroyed { self_id: self.id });
    }

    pub fn schedule_realloc(self: &Rc<Self>) {
        self.need_realloc.set(true);
        if !self.realloc_scheduled.replace(true) {
            self.client
                .state
                .pending_toplevel_screencast_reallocs
                .push(self.clone());
        }
    }

    fn realloc(&self, ctx: &Rc<dyn GfxContext>) -> Result<(), JayScreencastError> {
        if !self.destroyed.get() && self.buffers_acked.get() {
            self.do_realloc(ctx)
        } else {
            Ok(())
        }
    }

    fn do_realloc(&self, ctx: &Rc<dyn GfxContext>) -> Result<(), JayScreencastError> {
        self.need_realloc.set(false);
        let mut buffers = vec![];
        let formats = ctx.formats();
        let format = match formats.get(&XRGB8888.drm) {
            Some(f) => f,
            _ => return Err(JayScreencastError::XRGB8888),
        };
        if let Some(target) = self.target.get() {
            let (width, height) = target_size(Some(&target));
            let num = 3;
            for _ in 0..num {
                if width == 0 || height == 0 {
                    continue;
                }
                let mut usage = BO_USE_RENDERING;
                let modifiers = match self.linear.get() {
                    true if format.write_modifiers.contains(&LINEAR_MODIFIER) => {
                        vec![LINEAR_MODIFIER]
                    }
                    true if format.write_modifiers.contains(&INVALID_MODIFIER) => {
                        usage |= BO_USE_LINEAR;
                        vec![INVALID_MODIFIER]
                    }
                    true => return Err(JayScreencastError::Modifier),
                    false if format.write_modifiers.is_empty() => {
                        return Err(JayScreencastError::XRGB8888Writing)
                    }
                    false => format.write_modifiers.iter().copied().collect(),
                };
                let buffer = ctx.allocator().create_bo(
                    &self.client.state.dma_buf_ids,
                    width,
                    height,
                    XRGB8888,
                    &modifiers,
                    usage,
                )?;
                let fb = ctx.clone().dmabuf_img(buffer.dmabuf())?.to_framebuffer()?;
                buffers.push(ScreencastBuffer {
                    dmabuf: buffer.dmabuf().clone(),
                    _bo: buffer,
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
        if let Some(target) = self.target.get() {
            let rect = match target {
                Target::Output(o) => o.global.pos.get(),
                Target::Toplevel(t) => {
                    if !t.node_visible() {
                        return;
                    }
                    t.node_absolute_position()
                }
            };
            self.client.state.damage(rect);
        }
    }

    pub fn update_latch_listener(&self) {
        let Some(Target::Toplevel(tl)) = self.target.get() else {
            return;
        };
        let data = tl.tl_data();
        if data.visible.get() {
            self.latch_listener.attach(&data.output().latch_event);
        } else {
            self.latch_listener.detach();
        }
    }
}

impl JayScreencastRequestHandler for JayScreencast {
    type Error = JayScreencastError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_output(&self, req: SetOutput, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = if req.output.is_some() {
            Some(PendingTarget::Output(self.client.lookup(req.output)?))
        } else {
            None
        };
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.target.set(Some(output));
        Ok(())
    }

    fn set_allow_all_workspaces(
        &self,
        req: SetAllowAllWorkspaces,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.show_all.set(Some(req.allow_all != 0));
        Ok(())
    }

    fn allow_workspace(&self, req: AllowWorkspace, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
        _req: TouchAllowedWorkspaces,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending
            .show_workspaces
            .borrow_mut()
            .get_or_insert_default_ext();
        Ok(())
    }

    fn set_use_linear_buffers(
        &self,
        req: SetUseLinearBuffers,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.linear.set(Some(req.use_linear != 0));
        Ok(())
    }

    fn set_running(&self, req: SetRunning, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.running.set(Some(req.running != 0));
        Ok(())
    }

    fn configure(&self, _req: Configure, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }

        let mut need_realloc = false;

        if let Some(target) = self.pending.target.take() {
            self.detach();
            let mut new_target = None;
            if let Some(new) = target {
                match new {
                    PendingTarget::Output(o) => {
                        let Some(o) = o.output.node() else {
                            self.do_destroy();
                            return Ok(());
                        };
                        o.add_screencast(slf);
                        new_target = Some(Target::Output(o));
                    }
                    PendingTarget::Toplevel(t) => {
                        if t.destroyed.get() {
                            self.do_destroy();
                            return Ok(());
                        }
                        let t = t.toplevel.clone();
                        let data = t.tl_data();
                        data.jay_screencasts
                            .set((self.client.id, self.id), slf.clone());
                        if data.visible.get() {
                            self.latch_listener.attach(&data.output().latch_event);
                        }
                        new_target = Some(Target::Toplevel(t));
                    }
                }
            }
            if target_size(new_target.as_ref()) != target_size(self.target.get().as_ref()) {
                need_realloc = true;
            }
            self.target.set(new_target);
        }
        if let Some(linear) = self.pending.linear.take() {
            if self.linear.replace(linear) != linear {
                need_realloc = true;
            }
        }
        let mut capture_rules_changed = false;
        if let Some(show_all) = self.pending.show_all.take() {
            self.show_all.set(show_all);
            capture_rules_changed = true;
        }
        if let Some(new_workspaces) = self.pending.show_workspaces.borrow_mut().take() {
            *self.show_workspaces.borrow_mut() = new_workspaces;
            capture_rules_changed = true;
        }
        if let Some(running) = self.pending.running.take() {
            self.running.set(running);
        }

        if need_realloc {
            slf.schedule_realloc();
        }

        if capture_rules_changed {
            if let Some(Target::Output(o)) = self.target.get() {
                o.screencast_changed();
            }
        }

        if self.running.get() {
            self.damage();
        }

        Ok(())
    }

    fn ack_buffers(&self, req: AckBuffers, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }
        if req.serial == self.buffers_serial.get() {
            self.buffers_acked.set(true);
            if self.need_realloc.get() {
                slf.schedule_realloc();
            }
        }
        Ok(())
    }

    fn ack_config(&self, req: AckConfig, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }
        if req.serial == self.config_serial.get() {
            self.config_acked.set(true);
        }
        Ok(())
    }

    fn release_buffer(&self, req: ReleaseBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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

    fn set_toplevel(&self, req: SetToplevel, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let toplevel = if req.id.is_some() {
            Some(PendingTarget::Toplevel(self.client.lookup(req.id)?))
        } else {
            None
        };
        if self.destroyed.get() || !self.config_acked.get() {
            return Ok(());
        }
        self.pending.target.set(Some(toplevel));
        Ok(())
    }
}

object_base! {
    self = JayScreencast;
    version = Version(1);
}

impl Object for JayScreencast {
    fn break_loops(&self) {
        self.detach();
    }
}

dedicated_add_obj!(JayScreencast, JayScreencastId, screencasts);

#[derive(Debug, Error)]
pub enum JayScreencastError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Buffer index {0} is out-of-bounds")]
    OutOfBounds(u32),
    #[error(transparent)]
    AllocatorError(#[from] AllocatorError),
    #[error(transparent)]
    GfxError(#[from] GfxError),
    #[error("Render context does not support XRGB8888 format")]
    XRGB8888,
    #[error("Render context does not support XRGB8888 format for rendering")]
    XRGB8888Writing,
    #[error("Render context supports neither linear or invalid modifier")]
    Modifier,
}
efrom!(JayScreencastError, ClientError);

fn target_size(target: Option<&Target>) -> (i32, i32) {
    if let Some(target) = target {
        match target {
            Target::Output(o) => return o.global.pixel_size(),
            Target::Toplevel(t) => {
                let data = t.tl_data();
                let (dw, dh) = data.desired_extents.get().size();
                if let Some(ws) = data.workspace.get() {
                    let scale = ws.output.get().global.persistent.scale.get();
                    return scale.pixel_size(dw, dh);
                };
            }
        }
    }
    (0, 0)
}
