use {
    crate::{
        allocator::{AllocatorError, BO_USE_LINEAR, BO_USE_RENDERING, BufferObject},
        client::{Client, ClientError},
        cmm::cmm_description::ColorDescription,
        format::XRGB8888,
        gfx_api::{
            AcquireSync, BufferResv, GfxContext, GfxError, GfxFramebuffer, GfxTexture, ReleaseSync,
        },
        ifs::{jay_output::JayOutput, jay_toplevel::JayToplevel, wl_buffer::WlBufferStorage},
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
        video::{INVALID_MODIFIER, LINEAR_MODIFIER, dmabuf::DmaBuf},
        wire::{JayScreencastId, jay_screencast::*},
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
        let screencast = state
            .pending_screencast_reallocs_or_reconfigures
            .pop()
            .await;
        screencast.realloc_or_reconfigure_scheduled.set(false);
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

pub const CLIENT_BUFFERS_SINCE: Version = Version(7);

pub struct JayScreencast {
    pub id: JayScreencastId,
    pub version: Version,
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
    need_realloc_or_reconfigure: Cell<bool>,
    realloc_or_reconfigure_scheduled: Cell<bool>,
    latch_listener: EventListener<dyn LatchListener>,
}

#[derive(Clone)]
enum Target {
    Output(Rc<OutputNode>),
    Toplevel(Rc<dyn ToplevelNode>),
}

impl LatchListener for JayScreencast {
    fn after_latch(self: Rc<Self>, _on: &OutputNode, _tearing: bool) {
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
    clear_buffers: Cell<bool>,
    buffers: RefCell<Vec<Rc<dyn GfxFramebuffer>>>,
}

struct ScreencastBuffer {
    _bo: Option<Rc<dyn BufferObject>>,
    dmabuf: Option<DmaBuf>,
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

    pub fn new(
        id: JayScreencastId,
        client: &Rc<Client>,
        slf: &Weak<Self>,
        version: Version,
    ) -> Self {
        Self {
            id,
            version,
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
            need_realloc_or_reconfigure: Cell::new(false),
            realloc_or_reconfigure_scheduled: Cell::new(false),
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
                    AcquireSync::Implicit,
                    ReleaseSync::Implicit,
                    self.client.state.color_manager.srgb_gamma22(),
                    &*tl,
                    &self.client.state,
                    Some(tl.node_mapped_position()),
                    scale,
                    true,
                    true,
                    false,
                    false,
                    Transform::None,
                    None,
                    self.client.state.color_manager.srgb_linear(),
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
            let Some(dmabuf) = &buffer.dmabuf else {
                log::error!("Trying to send buffers but buffers are client allocated");
                self.do_destroy();
                return;
            };
            for plane in &dmabuf.planes {
                self.client.event(Plane {
                    self_id: self.id,
                    fd: plane.fd.clone(),
                    offset: plane.offset,
                    stride: plane.stride,
                });
            }
            self.client.event(Buffer {
                self_id: self.id,
                format: dmabuf.format.drm,
                modifier: dmabuf.modifier,
                width: dmabuf.width,
                height: dmabuf.height,
            });
        }
        self.client.event(BuffersDone {
            self_id: self.id,
            serial,
        });
    }

    fn send_config(&self) {
        self.need_realloc_or_reconfigure.set(false);
        self.config_acked.set(false);
        let serial = self.config_serial.fetch_add(1) + 1;
        if let Some(target) = self.target.get() {
            let (width, height) = target_size(Some(&target));
            if self.version >= CLIENT_BUFFERS_SINCE {
                self.client.event(ConfigSize {
                    self_id: self.id,
                    width,
                    height,
                });
            }
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
        cd: &Rc<ColorDescription>,
        resv: Option<&Rc<dyn BufferResv>>,
        acquire_sync: &AcquireSync,
        release_sync: ReleaseSync,
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
                    resv,
                    acquire_sync,
                    release_sync,
                    cd,
                    &buffer.fb,
                    AcquireSync::Implicit,
                    ReleaseSync::Implicit,
                    Transform::None,
                    self.client.state.color_manager.srgb_gamma22(),
                    on.global.pos.get(),
                    render_hardware_cursors,
                    x_off,
                    y_off,
                    size,
                    on.global.persistent.transform.get(),
                    on.global.persistent.scale.get(),
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
        self.buffers.borrow_mut().clear();
        self.destroyed.set(true);
        self.client.event(Destroyed { self_id: self.id });
    }

    pub fn schedule_realloc_or_reconfigure(self: &Rc<Self>) {
        self.need_realloc_or_reconfigure.set(true);
        if !self.realloc_or_reconfigure_scheduled.replace(true) {
            self.client
                .state
                .pending_screencast_reallocs_or_reconfigures
                .push(self.clone());
        }
    }

    fn realloc(&self, ctx: &Rc<dyn GfxContext>) -> Result<(), JayScreencastError> {
        if self.destroyed.get() {
            return Ok(());
        }
        if self.version < CLIENT_BUFFERS_SINCE {
            if self.buffers_acked.get() {
                return self.do_realloc(ctx);
            }
        } else {
            if self.config_acked.get() {
                self.send_config();
            }
        }
        Ok(())
    }

    fn do_realloc(&self, ctx: &Rc<dyn GfxContext>) -> Result<(), JayScreencastError> {
        self.need_realloc_or_reconfigure.set(false);
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
                    true if format.write_modifiers.contains_key(&LINEAR_MODIFIER) => {
                        vec![LINEAR_MODIFIER]
                    }
                    true if format.write_modifiers.contains_key(&INVALID_MODIFIER) => {
                        usage |= BO_USE_LINEAR;
                        vec![INVALID_MODIFIER]
                    }
                    true => return Err(JayScreencastError::Modifier),
                    false if format.write_modifiers.is_empty() => {
                        return Err(JayScreencastError::XRGB8888Writing);
                    }
                    false => format.write_modifiers.keys().copied().collect(),
                };
                let buffer = ctx.allocator().create_bo(
                    &self.client.state.dma_buf_ids,
                    width,
                    height,
                    format.format,
                    &modifiers,
                    usage,
                )?;
                let fb = ctx.clone().dmabuf_img(buffer.dmabuf())?.to_framebuffer()?;
                buffers.push(ScreencastBuffer {
                    dmabuf: Some(buffer.dmabuf().clone()),
                    _bo: Some(buffer),
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
                    t.node_mapped_position()
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
        if self.destroyed.get() {
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
        if self.destroyed.get() {
            return Ok(());
        }
        self.pending.show_all.set(Some(req.allow_all != 0));
        Ok(())
    }

    fn allow_workspace(&self, req: AllowWorkspace, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let ws = self.client.lookup(req.workspace)?;
        if self.destroyed.get() {
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
        if self.destroyed.get() {
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
        if self.destroyed.get() {
            return Ok(());
        }
        self.pending.linear.set(Some(req.use_linear != 0));
        Ok(())
    }

    fn set_running(&self, req: SetRunning, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }
        self.pending.running.set(Some(req.running != 0));
        Ok(())
    }

    fn configure(&self, _req: Configure, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }

        let mut need_realloc_or_reconfigure = false;

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
                need_realloc_or_reconfigure = true;
            }
            self.target.set(new_target);
        }
        if let Some(linear) = self.pending.linear.take() {
            if self.linear.replace(linear) != linear {
                need_realloc_or_reconfigure = true;
            }
        }
        if self.pending.clear_buffers.take() {
            self.buffers.borrow_mut().clear();
        }
        for buffer in self.pending.buffers.borrow_mut().drain(..) {
            self.buffers.borrow_mut().push(ScreencastBuffer {
                _bo: None,
                dmabuf: None,
                fb: buffer,
                free: true,
            });
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

        if need_realloc_or_reconfigure {
            slf.schedule_realloc_or_reconfigure();
        }

        if capture_rules_changed && let Some(Target::Output(o)) = self.target.get() {
            o.screencast_changed();
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
            if self.need_realloc_or_reconfigure.get() {
                slf.schedule_realloc_or_reconfigure();
            }
        }
        Ok(())
    }

    fn ack_config(&self, req: AckConfig, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }
        if req.serial == self.config_serial.get() {
            self.config_acked.set(true);
            if self.need_realloc_or_reconfigure.get() {
                slf.schedule_realloc_or_reconfigure();
            }
        }
        Ok(())
    }

    fn release_buffer(&self, req: ReleaseBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() || !self.buffers_acked.get() {
            return Ok(());
        }
        let idx = req.idx as usize;
        if idx >= self.buffers.borrow_mut().len() {
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
        if self.destroyed.get() {
            return Ok(());
        }
        self.pending.target.set(Some(toplevel));
        Ok(())
    }

    fn clear_buffers(&self, _req: ClearBuffers, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }
        self.pending.clear_buffers.set(true);
        Ok(())
    }

    fn add_buffer(&self, req: AddBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.destroyed.get() {
            return Ok(());
        }
        let buffer = self.client.lookup(req.buffer)?;
        if let Some(WlBufferStorage::Dmabuf { img, .. }) = &*buffer.storage.borrow() {
            match img.clone().to_framebuffer() {
                Ok(fb) => self.pending.buffers.borrow_mut().push(fb),
                Err(e) => {
                    log::warn!(
                        "Could not turn GfxImage into GfxFramebuffer: {}",
                        ErrorFmt(e)
                    );
                    self.do_destroy();
                }
            }
            return Ok(());
        }
        Err(JayScreencastError::NotDmabuf)
    }
}

object_base! {
    self = JayScreencast;
    version = self.version;
}

impl Object for JayScreencast {
    fn break_loops(self: Rc<Self>) {
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
    #[error("Buffer is not a dmabuf")]
    NotDmabuf,
}
efrom!(JayScreencastError, ClientError);

fn target_size(target: Option<&Target>) -> (i32, i32) {
    if let Some(target) = target {
        return match target {
            Target::Output(o) => o.global.pixel_size(),
            Target::Toplevel(t) => t.tl_data().desired_pixel_size(),
        };
    }
    (0, 0)
}
