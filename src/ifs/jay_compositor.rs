use {
    crate::{
        cli::CliLogLevel,
        client::{Client, ClientCaps, ClientError, CAP_JAY_COMPOSITOR},
        globals::{Global, GlobalName},
        ifs::{
            jay_ei_session_builder::JayEiSessionBuilder,
            jay_idle::JayIdle,
            jay_input::JayInput,
            jay_log_file::JayLogFile,
            jay_output::JayOutput,
            jay_pointer::JayPointer,
            jay_randr::JayRandr,
            jay_render_ctx::JayRenderCtx,
            jay_screencast::JayScreencast,
            jay_screenshot::JayScreenshot,
            jay_seat_events::JaySeatEvents,
            jay_select_toplevel::{JaySelectToplevel, JayToplevelSelector},
            jay_select_workspace::{JaySelectWorkspace, JayWorkspaceSelector},
            jay_workspace_watcher::JayWorkspaceWatcher,
            jay_xwayland::JayXwayland,
        },
        leaks::Tracker,
        object::{Object, Version},
        screenshoter::take_screenshot,
        utils::{errorfmt::ErrorFmt, toplevel_identifier::ToplevelIdentifier},
        wire::{jay_compositor::*, JayCompositorId, JayScreenshotId},
    },
    bstr::ByteSlice,
    log::Level,
    std::{cell::Cell, ops::Deref, rc::Rc, str::FromStr},
    thiserror::Error,
};

pub const CREATE_EI_SESSION_SINCE: Version = Version(5);
pub const SCREENSHOT_SPLITUP_SINCE: Version = Version(6);
pub const GET_TOPLEVEL_SINCE: Version = Version(12);

pub struct JayCompositorGlobal {
    name: GlobalName,
}

impl JayCompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayCompositorId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), JayCompositorError> {
        let obj = Rc::new(JayCompositor {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        obj.send_capabilities();
        Ok(())
    }
}

global_base!(JayCompositorGlobal, JayCompositor, JayCompositorError);

impl Global for JayCompositorGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        12
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_JAY_COMPOSITOR
    }
}

simple_add_global!(JayCompositorGlobal);

pub struct JayCompositor {
    id: JayCompositorId,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

pub struct Cap;

impl Cap {
    pub const NONE: u16 = 0;
    pub const WINDOW_CAPTURE: u16 = 1;
    pub const SELECT_WORKSPACE: u16 = 2;
}

impl JayCompositor {
    fn send_capabilities(&self) {
        self.client.event(Capabilities {
            self_id: self.id,
            cap: &[Cap::NONE, Cap::WINDOW_CAPTURE, Cap::SELECT_WORKSPACE],
        });
    }

    fn take_screenshot_impl(
        &self,
        id: JayScreenshotId,
        include_cursor: bool,
    ) -> Result<(), JayCompositorError> {
        let ss = Rc::new(JayScreenshot {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
        });
        track!(self.client, ss);
        self.client.add_client_obj(&ss)?;
        match take_screenshot(&self.client.state, include_cursor) {
            Ok(s) => {
                let dmabuf = s.bo.dmabuf();
                if self.version < SCREENSHOT_SPLITUP_SINCE {
                    if let Some(drm) = &s.drm {
                        let plane = &dmabuf.planes[0];
                        ss.send_dmabuf(
                            drm,
                            &plane.fd,
                            dmabuf.width,
                            dmabuf.height,
                            plane.offset,
                            plane.stride,
                            dmabuf.modifier,
                        );
                    } else {
                        ss.send_error("Buffer has no associated DRM device");
                    }
                } else {
                    if let Some(drm) = &s.drm {
                        ss.send_drm_dev(drm);
                    }
                    for plane in &dmabuf.planes {
                        ss.send_plane(plane);
                    }
                    ss.send_dmabuf2(dmabuf);
                }
            }
            Err(e) => {
                let msg = ErrorFmt(e).to_string();
                ss.send_error(&msg);
            }
        }
        self.client.remove_obj(ss.deref())?;
        Ok(())
    }
}

impl JayCompositorRequestHandler for JayCompositor {
    type Error = JayCompositorError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_log_file(&self, req: GetLogFile, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let log_file = Rc::new(JayLogFile::new(req.id, &self.client));
        track!(self.client, log_file);
        self.client.add_client_obj(&log_file)?;
        match &self.client.state.logger {
            Some(logger) => log_file.send_path(logger.path().as_bstr()),
            _ => log_file.send_path(b"".as_bstr()),
        };
        Ok(())
    }

    fn quit(&self, _req: Quit, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        log::info!("Quitting");
        self.client.state.ring.stop();
        Ok(())
    }

    fn set_log_level(&self, req: SetLogLevel, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        const ERROR: u32 = CliLogLevel::Error as u32;
        const WARN: u32 = CliLogLevel::Warn as u32;
        const INFO: u32 = CliLogLevel::Info as u32;
        const DEBUG: u32 = CliLogLevel::Debug as u32;
        const TRACE: u32 = CliLogLevel::Trace as u32;
        let level = match req.level {
            ERROR => Level::Error,
            WARN => Level::Warn,
            INFO => Level::Info,
            DEBUG => Level::Debug,
            TRACE => Level::Trace,
            _ => return Err(JayCompositorError::UnknownLogLevel(req.level)),
        };
        if let Some(logger) = &self.client.state.logger {
            logger.set_level(level);
        }
        Ok(())
    }

    fn take_screenshot(&self, req: TakeScreenshot, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.take_screenshot_impl(req.id, false)
    }

    fn take_screenshot2(&self, req: TakeScreenshot2, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.take_screenshot_impl(req.id, req.include_cursor != 0)
    }

    fn get_idle(&self, req: GetIdle, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let idle = Rc::new(JayIdle {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
        });
        track!(self.client, idle);
        self.client.add_client_obj(&idle)?;
        Ok(())
    }

    fn get_client_id(&self, _req: GetClientId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.event(ClientId {
            self_id: self.id,
            client_id: self.client.id.raw(),
        });
        Ok(())
    }

    fn enable_symmetric_delete(
        &self,
        _req: EnableSymmetricDelete,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.client.symmetric_delete.set(true);
        Ok(())
    }

    fn unlock(&self, _req: Unlock, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let state = &self.client.state;
        if state.lock.locked.get() {
            if let Some(lock) = state.lock.lock.get() {
                lock.finish();
            }
            state.do_unlock();
        }
        Ok(())
    }

    fn get_seats(&self, _req: GetSeats, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        for seat in self.client.state.globals.seats.lock().values() {
            self.client.event(Seat {
                self_id: self.id,
                id: seat.id().raw(),
                name: seat.seat_name(),
            })
        }
        Ok(())
    }

    fn seat_events(&self, req: SeatEvents, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let se = Rc::new(JaySeatEvents {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
        });
        track!(self.client, se);
        self.client.add_client_obj(&se)?;
        self.client
            .state
            .testers
            .borrow_mut()
            .insert((self.client.id, req.id), se);
        Ok(())
    }

    fn get_output(&self, req: GetOutput, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = self.client.lookup(req.output)?;
        let jo = Rc::new(JayOutput {
            id: req.id,
            client: self.client.clone(),
            output: output.global.clone(),
            tracker: Default::default(),
        });
        track!(self.client, jo);
        self.client.add_client_obj(&jo)?;
        if let Some(node) = jo.output.node() {
            node.jay_outputs.set((self.client.id, req.id), jo.clone());
            jo.send_linear_id();
        } else {
            jo.send_destroyed();
        }
        Ok(())
    }

    fn get_pointer(&self, req: GetPointer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let ctx = Rc::new(JayPointer {
            id: req.id,
            client: self.client.clone(),
            seat: seat.global.clone(),
            tracker: Default::default(),
        });
        track!(self.client, ctx);
        self.client.add_client_obj(&ctx)?;
        Ok(())
    }

    fn get_render_ctx(&self, req: GetRenderCtx, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let ctx = Rc::new(JayRenderCtx {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, ctx);
        self.client.add_client_obj(&ctx)?;
        self.client
            .state
            .render_ctx_watchers
            .set((self.client.id, req.id), ctx.clone());
        let rctx = self.client.state.render_ctx.get();
        ctx.send_render_ctx(rctx);
        Ok(())
    }

    fn watch_workspaces(&self, req: WatchWorkspaces, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let watcher = Rc::new(JayWorkspaceWatcher {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
        });
        track!(self.client, watcher);
        self.client.add_client_obj(&watcher)?;
        self.client
            .state
            .workspace_watchers
            .set((self.client.id, req.id), watcher.clone());
        for ws in self.client.state.workspaces.lock().values() {
            watcher.send_workspace(ws)?;
        }
        Ok(())
    }

    fn create_screencast(&self, req: CreateScreencast, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let sc = Rc::new_cyclic(|slf| JayScreencast::new(req.id, &self.client, slf, self.version));
        track!(self.client, sc);
        self.client.add_client_obj(&sc)?;
        Ok(())
    }

    fn get_randr(&self, req: GetRandr, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let sc = Rc::new(JayRandr::new(req.id, &self.client, self.version));
        track!(self.client, sc);
        self.client.add_client_obj(&sc)?;
        Ok(())
    }

    fn get_input(&self, req: GetInput, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let sc = Rc::new(JayInput::new(req.id, &self.client, self.version));
        track!(self.client, sc);
        self.client.add_client_obj(&sc)?;
        Ok(())
    }

    fn select_toplevel(&self, req: SelectToplevel, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let obj = JaySelectToplevel::new(&self.client, req.id, self.version);
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        let selector = JayToplevelSelector {
            tl: Default::default(),
            jst: obj.clone(),
        };
        seat.global.select_toplevel(selector);
        Ok(())
    }

    fn select_workspace(&self, req: SelectWorkspace, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let obj = Rc::new(JaySelectWorkspace {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            destroyed: Cell::new(false),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        let selector = JayWorkspaceSelector {
            ws: Default::default(),
            jsw: obj.clone(),
        };
        seat.global.select_workspace(selector);
        Ok(())
    }

    fn create_ei_session(&self, req: CreateEiSession, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(JayEiSessionBuilder {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            app_id: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn get_xwayland(&self, req: GetXwayland, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(JayXwayland {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn get_toplevel(&self, req: GetToplevel<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = JaySelectToplevel::new(&self.client, req.id, self.version);
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        let tl = match ToplevelIdentifier::from_str(req.toplevel_id) {
            Ok(id) => self
                .client
                .state
                .toplevels
                .get(&id)
                .and_then(|w| w.upgrade()),
            Err(e) => {
                log::error!("Could not parse toplevel id: {}", ErrorFmt(e));
                None
            }
        };
        obj.done(tl);
        Ok(())
    }
}

object_base! {
    self = JayCompositor;
    version = self.version;
}

impl Object for JayCompositor {}

simple_add_obj!(JayCompositor);

#[derive(Debug, Error)]
pub enum JayCompositorError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Unknown log level {0}")]
    UnknownLogLevel(u32),
}
efrom!(JayCompositorError, ClientError);
