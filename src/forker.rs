mod io;

use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        compositor::{DISPLAY, LIBEI_SOCKET, WAYLAND_DISPLAY},
        forker::io::{IoIn, IoOut},
        io_uring::IoUring,
        state::State,
        utils::{
            buffd::BufFdError,
            clone3::{fork_with_pidfd, Forked},
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            process_name::set_process_name,
            queue::AsyncQueue,
        },
        xwayland,
    },
    ahash::AHashMap,
    bincode::Options,
    jay_config::_private::bincode_ops,
    log::Level,
    serde::{Deserialize, Serialize},
    std::{
        cell::{Cell, RefCell},
        env,
        ffi::OsStr,
        io::{Read, Write},
        os::unix::ffi::OsStrExt,
        rc::{Rc, Weak},
        task::{Poll, Waker},
    },
    thiserror::Error,
    uapi::{c, pipe2, Errno, Fd, IntoUstr, OwnedFd, UstrPtr},
};

pub struct ForkerProxy {
    pidfd: Rc<OwnedFd>,
    pid: c::pid_t,
    socket: Rc<OwnedFd>,
    task_in: Cell<Option<SpawnedFuture<()>>>,
    task_out: Cell<Option<SpawnedFuture<()>>>,
    task_proc: Cell<Option<SpawnedFuture<()>>>,
    outgoing: AsyncQueue<ServerMessage>,
    next_id: NumCell<u32>,
    pending_pidfds: CopyHashMap<u32, Weak<PidfdHandoff>>,
    fds: RefCell<Vec<Rc<OwnedFd>>>,
}

struct PidfdHandoff {
    pidfd: Cell<Option<Result<(Rc<OwnedFd>, c::pid_t), ForkerError>>>,
    waiter: Cell<Option<Waker>>,
}

#[derive(Debug, Error)]
pub enum ForkerError {
    #[error("Could not create a socketpair")]
    Socketpair(#[source] crate::utils::oserror::OsError),
    #[error("Could not fork")]
    Fork(#[source] crate::utils::oserror::OsError),
    #[error("Could not read the next message")]
    ReadFailed(#[source] BufFdError),
    #[error("Could not write the next message")]
    WriteFailed(#[source] BufFdError),
    #[error("Could not decode the next message")]
    DecodeFailed(#[source] bincode::Error),
    #[error("Could not encode the next message")]
    EncodeFailed(#[source] bincode::Error),
    #[error("Could not fork")]
    PidfdForkFailed,
}

impl ForkerProxy {
    pub fn clear(&self) {
        self.task_in.take();
        self.task_out.take();
        self.task_proc.take();
        self.outgoing.clear();
    }

    pub fn create() -> Result<Self, ForkerError> {
        let (parent, child) =
            match uapi::socketpair(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0) {
                Ok(o) => o,
                Err(e) => return Err(ForkerError::Socketpair(e.into())),
            };
        let pid = uapi::getpid();
        match fork_with_pidfd(false)? {
            Forked::Parent { pid, pidfd } => Ok(ForkerProxy {
                pidfd: Rc::new(pidfd),
                pid,
                socket: Rc::new(parent),
                task_in: Cell::new(None),
                task_out: Cell::new(None),
                task_proc: Cell::new(None),
                outgoing: Default::default(),
                next_id: Default::default(),
                pending_pidfds: Default::default(),
                fds: Default::default(),
            }),
            Forked::Child { .. } => {
                drop(parent);
                Forker::handle(pid, child)
            }
        }
    }

    pub fn install(self: &Rc<Self>, state: &Rc<State>) {
        state.forker.set(Some(self.clone()));
        self.task_proc.set(Some(
            state.eng.spawn(self.clone().check_process(state.clone())),
        ));
        self.task_in
            .set(Some(state.eng.spawn(self.clone().incoming(state.clone()))));
        self.task_out
            .set(Some(state.eng.spawn(self.clone().outgoing(state.clone()))));
    }

    pub fn setenv(&self, key: &[u8], val: &[u8]) {
        self.outgoing.push(ServerMessage::SetEnv {
            var: key.to_vec(),
            val: Some(val.to_vec()),
        })
    }

    pub fn unsetenv(&self, key: &[u8]) {
        self.outgoing.push(ServerMessage::SetEnv {
            var: key.to_vec(),
            val: None,
        })
    }

    async fn pidfd(&self, id: u32) -> Result<(Rc<OwnedFd>, c::pid_t), ForkerError> {
        let handoff = Rc::new(PidfdHandoff {
            pidfd: Cell::new(None),
            waiter: Cell::new(None),
        });
        self.pending_pidfds.set(id, Rc::downgrade(&handoff));
        futures_util::future::poll_fn(|ctx| {
            if let Some(pidfd) = handoff.pidfd.take() {
                Poll::Ready(pidfd)
            } else {
                handoff.waiter.set(Some(ctx.waker().clone()));
                Poll::Pending
            }
        })
        .await
    }

    pub async fn xwayland(
        &self,
        state: &State,
        stderr: Rc<OwnedFd>,
        dfd: Rc<OwnedFd>,
        listenfd: Rc<OwnedFd>,
        wmfd: Rc<OwnedFd>,
        waylandfd: Rc<OwnedFd>,
    ) -> Result<(Rc<OwnedFd>, c::pid_t), ForkerError> {
        let (prog, args) = xwayland::build_args(state, self).await;
        let env = vec![
            ("WAYLAND_SOCKET".to_string(), Some("6".to_string())),
            (LIBEI_SOCKET.to_string(), None),
        ];
        let fds = vec![
            (2, stderr),
            (3, dfd),
            (4, listenfd),
            (5, wmfd),
            (6, waylandfd),
        ];
        let pidfd_id = self.next_id.fetch_add(1);
        self.spawn_(prog, args, env, fds, Some(pidfd_id));
        self.pidfd(pidfd_id).await
    }

    pub fn spawn(
        &self,
        prog: String,
        args: Vec<String>,
        env: Vec<(String, Option<String>)>,
        fds: Vec<(i32, Rc<OwnedFd>)>,
    ) {
        self.spawn_(prog, args, env, fds, None)
    }

    fn spawn_(
        &self,
        prog: String,
        args: Vec<String>,
        env: Vec<(String, Option<String>)>,
        fds: Vec<(i32, Rc<OwnedFd>)>,
        pidfd_id: Option<u32>,
    ) {
        for (_, fd) in &fds {
            self.fds.borrow_mut().push(fd.clone());
        }
        let fds = fds.into_iter().map(|(a, _)| a).collect();
        self.outgoing.push(ServerMessage::Spawn {
            prog,
            args,
            env,
            fds,
            pidfd_id,
        })
    }

    async fn incoming(self: Rc<Self>, state: Rc<State>) {
        let mut io = IoIn::new(&self.socket, &state.ring);
        loop {
            let msg = match io.read_msg().await {
                Ok(msg) => msg,
                Err(e) => {
                    log::error!("Could not read from the ol' forker: {}", ErrorFmt(e));
                    self.task_in.take();
                    return;
                }
            };
            self.handle_msg(msg, &mut io);
        }
    }

    fn handle_msg(&self, msg: ForkerMessage, io: &mut IoIn) {
        match msg {
            ForkerMessage::Log { level, msg } => self.handle_log(level, &msg),
            ForkerMessage::PidFd { id, success, pid } => self.handle_pidfd(id, success, io, pid),
        }
    }

    fn handle_pidfd(&self, id: u32, success: bool, io: &mut IoIn, pid: c::pid_t) {
        let res = match success {
            true => Ok((io.pop_fd().unwrap(), pid)),
            _ => Err(ForkerError::PidfdForkFailed),
        };
        if let Some(handoff) = self.pending_pidfds.remove(&id) {
            if let Some(handoff) = handoff.upgrade() {
                handoff.pidfd.set(Some(res));
                if let Some(w) = handoff.waiter.take() {
                    w.wake();
                }
            }
        }
    }

    fn handle_log(&self, level: usize, msg: &str) {
        let level = match level {
            1 => Level::Error,
            2 => Level::Warn,
            3 => Level::Info,
            4 => Level::Debug,
            5 => Level::Trace,
            _ => Level::Error,
        };
        log::log!(level, "{}", msg);
    }

    async fn outgoing(self: Rc<Self>, state: Rc<State>) {
        let mut io = IoOut::new(&self.socket, &state.ring);
        loop {
            let msg = self.outgoing.pop().await;
            for fd in self.fds.borrow_mut().drain(..) {
                io.push_fd(fd);
            }
            if let Err(e) = io.write_msg(msg).await {
                log::error!("Could not write to the ol' forker: {}", ErrorFmt(e));
                self.clear();
                state.forker.set(None);
                return;
            }
        }
    }

    async fn check_process(self: Rc<Self>, state: Rc<State>) {
        if let Err(e) = state.ring.readable(&self.pidfd).await {
            log::error!(
                "Cannot wait for the forker pidfd to become readable: {}",
                ErrorFmt(e)
            );
        } else {
            let _ = uapi::waitpid(self.pid, 0);
        }
        log::error!("The ol' forker died. Cannot spawn further processes.");
        self.clear();
        state.forker.set(None);
    }
}

#[derive(Serialize, Deserialize)]
enum ServerMessage {
    SetEnv {
        var: Vec<u8>,
        val: Option<Vec<u8>>,
    },
    Spawn {
        prog: String,
        args: Vec<String>,
        env: Vec<(String, Option<String>)>,
        fds: Vec<i32>,
        pidfd_id: Option<u32>,
    },
}

#[derive(Serialize, Deserialize)]
enum ForkerMessage {
    Log {
        level: usize,
        msg: String,
    },
    PidFd {
        id: u32,
        success: bool,
        pid: c::pid_t,
    },
}

struct Forker {
    socket: Rc<OwnedFd>,
    ae: Rc<AsyncEngine>,
    ring: Rc<IoUring>,
    fds: RefCell<Vec<Rc<OwnedFd>>>,
    outgoing: AsyncQueue<ForkerMessage>,
    pending_spawns: CopyHashMap<c::pid_t, SpawnedFuture<()>>,
}

impl Forker {
    fn handle(ppid: c::pid_t, socket: OwnedFd) -> ! {
        env::set_var("XDG_SESSION_TYPE", "wayland");
        env::remove_var(DISPLAY);
        env::remove_var(WAYLAND_DISPLAY);
        set_process_name("the ol' forker");
        setup_deathsig(ppid);
        reset_signals();
        let socket = Rc::new(setup_fds(socket));
        std::panic::set_hook({
            let socket = socket.raw();
            Box::new(move |pi| {
                let msg = ForkerMessage::Log {
                    level: log::Level::Error as _,
                    msg: format!("The ol' forker panicked: {}", pi),
                };
                let msg = bincode_ops().serialize(&msg).unwrap();
                let _ = Fd::new(socket).write_all(&msg);
            })
        });
        let ae = AsyncEngine::new();
        let ring = IoUring::new(&ae, 32).unwrap();
        let forker = Rc::new(Forker {
            socket,
            ae: ae.clone(),
            ring: ring.clone(),
            fds: RefCell::new(vec![]),
            outgoing: Default::default(),
            pending_spawns: Default::default(),
        });
        let _f1 = ae.spawn(forker.clone().incoming());
        let _f2 = ae.spawn(forker.clone().outgoing());
        let _ = ring.run();
        std::process::exit(1);
    }

    async fn outgoing(self: Rc<Self>) {
        let mut io = IoOut::new(&self.socket, &self.ring);
        loop {
            let msg = self.outgoing.pop().await;
            for fd in self.fds.borrow_mut().drain(..) {
                io.push_fd(fd);
            }
            if io.write_msg(msg).await.is_err() {
                self.ring.stop();
                return;
            }
        }
    }

    async fn incoming(self: Rc<Self>) {
        let mut io = IoIn::new(&self.socket, &self.ring);
        loop {
            let msg = match io.read_msg().await {
                Ok(m) => m,
                _ => {
                    self.ring.stop();
                    return;
                }
            };
            self.handle_msg(msg, &mut io);
        }
    }

    fn handle_msg(self: &Rc<Self>, msg: ServerMessage, io: &mut IoIn) {
        match msg {
            ServerMessage::SetEnv { var, val } => self.handle_set_env(&var, val),
            ServerMessage::Spawn {
                prog,
                args,
                env,
                fds,
                pidfd_id,
            } => self.handle_spawn(prog, args, env, fds, io, pidfd_id),
        }
    }

    fn handle_set_env(self: &Rc<Self>, var: &[u8], val: Option<Vec<u8>>) {
        let var = OsStr::from_bytes(var);
        match val {
            Some(val) => env::set_var(var, OsStr::from_bytes(&val)),
            _ => env::remove_var(var),
        }
    }

    fn handle_spawn(
        self: &Rc<Self>,
        prog: String,
        args: Vec<String>,
        env: Vec<(String, Option<String>)>,
        fds: Vec<i32>,
        io: &mut IoIn,
        pidfd_id: Option<u32>,
    ) {
        let fds = fds
            .into_iter()
            .map(|a| (a, Rc::try_unwrap(io.pop_fd().unwrap()).unwrap()))
            .collect();
        self.spawn(prog, args, env, fds, pidfd_id)
    }

    fn spawn(
        self: &Rc<Self>,
        prog: String,
        args: Vec<String>,
        env: Vec<(String, Option<String>)>,
        fds: Vec<(i32, OwnedFd)>,
        pidfd_id: Option<u32>,
    ) {
        let (read, mut write) = pipe2(c::O_CLOEXEC).unwrap();
        let res = match fork_with_pidfd(false) {
            Ok(o) => o,
            Err(e) => {
                if let Some(id) = pidfd_id {
                    self.outgoing.push(ForkerMessage::PidFd {
                        id,
                        success: false,
                        pid: 0,
                    });
                }
                self.outgoing.push(ForkerMessage::Log {
                    level: log::Level::Error as usize,
                    msg: ErrorFmt(e).to_string(),
                });
                return;
            }
        };
        match res {
            Forked::Parent { pid, pidfd } => {
                if let Some(id) = pidfd_id {
                    self.fds.borrow_mut().push(Rc::new(pidfd));
                    self.outgoing.push(ForkerMessage::PidFd {
                        id,
                        success: true,
                        pid,
                    });
                }
                drop(write);
                let slf = self.clone();
                let spawn = self.ae.spawn(async move {
                    let read = Rc::new(read);
                    if let Err(e) = slf.ring.readable(&read).await {
                        log::error!(
                            "Cannot wait for the child fd to become readable: {}",
                            ErrorFmt(e)
                        );
                    } else {
                        let mut s = String::new();
                        let _ = Fd::new(read.raw()).read_to_string(&mut s);
                        if s.len() > 0 {
                            slf.outgoing.push(ForkerMessage::Log {
                                level: log::Level::Error as _,
                                msg: format!("Could not spawn `{}`: {}", prog, s),
                            });
                        }
                    }
                    slf.pending_spawns.remove(&pid);
                });
                self.pending_spawns.set(pid, spawn);
            }
            Forked::Child { .. } => {
                let err = (|| {
                    if let Some(max_desired) = fds.iter().map(|v| v.0).max() {
                        match uapi::fcntl_dupfd_cloexec(write.raw(), max_desired.wrapping_add(1)) {
                            Ok(new) => write = new,
                            Err(e) => return Err(SpawnError::Dupfd(e.into())),
                        }
                    }
                    let fds = map_fds(fds)?;
                    for fd in fds {
                        let fd = fd.unwrap();
                        let res: Result<_, Errno> = (|| {
                            uapi::fcntl_setfd(fd, uapi::fcntl_getfd(fd)? & !c::FD_CLOEXEC)?;
                            Ok(())
                        })();
                        if let Err(e) = res {
                            return Err(SpawnError::Cloexec(e.into()));
                        }
                    }
                    unsafe {
                        c::signal(c::SIGCHLD, c::SIG_DFL);
                    }
                    for (key, val) in env {
                        match val {
                            None => env::remove_var(&key),
                            Some(val) => env::set_var(&key, &val),
                        }
                    }
                    let prog = prog.into_ustr();
                    let mut argsnt = UstrPtr::new();
                    argsnt.push(&prog);
                    for arg in args {
                        argsnt.push(arg);
                    }
                    if let Err(e) = uapi::execvp(&prog, &argsnt) {
                        return Err(SpawnError::Exec(e.into()));
                    }
                    Ok(())
                })();
                if let Err(e) = err {
                    let _ = write.write_all(ErrorFmt(e).to_string().as_bytes());
                }
                std::process::exit(1);
            }
        }
    }
}

#[derive(Debug, Error)]
enum SpawnError {
    #[error("exec failed")]
    Exec(#[source] crate::utils::oserror::OsError),
    #[error("Could not unset cloexec flag")]
    Cloexec(#[source] crate::utils::oserror::OsError),
    #[error("dupfd faild")]
    Dupfd(#[source] crate::utils::oserror::OsError),
}

fn setup_fds(mut socket: OwnedFd) -> OwnedFd {
    if socket.raw() != 0 {
        uapi::dup3(socket.unwrap(), 0, 0).unwrap();
        socket = OwnedFd::new(0);
    }
    uapi::close_range(1, c::c_uint::MAX, 0).unwrap();
    uapi::dup3(socket.raw(), 3, c::O_CLOEXEC).unwrap();
    socket = OwnedFd::new(3);
    let fd = uapi::open("/dev/null", c::O_RDWR, 0).unwrap().unwrap();
    assert!(fd == 0);
    uapi::dup2(0, 1).unwrap();
    uapi::dup2(0, 2).unwrap();
    socket
}

fn reset_signals() {
    const NSIG: c::c_int = 64;
    unsafe {
        for sig in 1..=NSIG {
            c::signal(sig, c::SIG_DFL);
        }
        c::signal(c::SIGCHLD, c::SIG_IGN);
    }
}

fn setup_deathsig(ppid: c::pid_t) {
    unsafe {
        let res = c::prctl(c::PR_SET_PDEATHSIG, c::SIGKILL as c::c_ulong);
        uapi::map_err!(res).unwrap();
        if ppid != uapi::getppid() {
            std::process::exit(0);
        }
    }
}

fn map_fds(fds: Vec<(i32, OwnedFd)>) -> Result<Vec<OwnedFd>, SpawnError> {
    let mut desired: Vec<_> = fds.iter().map(|v| v.0).collect();
    desired.sort_by(|a, b| b.cmp(a));
    let mut existing_to_desired: AHashMap<_, _> = fds.iter().map(|v| (v.1.raw(), v.0)).collect();
    let mut desired_to_existing: AHashMap<_, _> = fds.into_iter().map(|v| (v.0, v.1)).collect();
    for desired in desired {
        let existing = desired_to_existing.get(&desired).unwrap().raw();
        if existing == desired {
            continue;
        }
        if let Some(conflict_desired) = existing_to_desired.get(&desired).copied() {
            match uapi::fcntl_dupfd_cloexec(desired, 0) {
                Ok(new) => {
                    existing_to_desired.remove(&desired);
                    existing_to_desired.insert(new.raw(), conflict_desired);
                    desired_to_existing.insert(conflict_desired, new);
                }
                Err(e) => return Err(SpawnError::Dupfd(e.into())),
            }
        }
        match uapi::dup3(existing, desired, c::O_CLOEXEC) {
            Ok(_) => {
                existing_to_desired.remove(&existing);
                existing_to_desired.insert(desired, desired);
                desired_to_existing.insert(desired, OwnedFd::new(desired));
            }
            Err(e) => return Err(SpawnError::Dupfd(e.into())),
        }
    }
    Ok(desired_to_existing.into_values().collect())
}
