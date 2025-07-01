use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        utils::{
            clone3::{Forked, fork_with_pidfd},
            errorfmt::ErrorFmt,
            oserror::OsError,
        },
        wire::{JayReexecId, jay_reexec::*},
    },
    std::{array::from_mut, cell::RefCell, rc::Rc},
    thiserror::Error,
    uapi::{OwnedFd, UstrPtr, c, close_range, dup2, pipe2, waitpid},
};

pub struct JayReexec {
    pub id: JayReexecId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub args: RefCell<Vec<String>>,
}

impl JayReexec {
    fn send_failed(&self, msg: &str) {
        self.client.event(Failed {
            self_id: self.id,
            msg,
        });
    }

    fn delay_close_input_fd(&self) -> Option<OwnedFd> {
        // It's 2025 and closing evdev fds is still abysmally slow.
        let mut fds = self.client.state.backend.get().get_input_fds();
        if fds.is_empty() {
            return None;
        }
        macro_rules! pipe {
            () => {
                match pipe2(c::O_CLOEXEC) {
                    Ok(p) => p,
                    Err(e) => {
                        log::error!("Could not create pipe: {}", ErrorFmt(OsError::from(e)));
                        return None;
                    }
                }
            };
        }
        let (p1, c1) = pipe!();
        let (c2, p2) = pipe!();
        if let Ok(f) = fork_with_pidfd(false) {
            match f {
                Forked::Parent { pid, .. } => {
                    let _ = waitpid(pid, 0);
                }
                Forked::Child { .. } => {
                    if let Ok(f) = fork_with_pidfd(false)
                        && let Forked::Child { .. } = f
                    {
                        drop(p2);
                        fds.sort_by_key(|fd| fd.raw());
                        let c2_dup = fds.last().unwrap().raw() + 1;
                        let c1_dup = c2_dup + 1;
                        let _ = dup2(c1.raw(), c1_dup);
                        let _ = dup2(c2.raw(), c2_dup);
                        for (idx, fd) in fds.iter().enumerate() {
                            let _ = dup2(fd.raw(), idx as _);
                        }
                        let c2_dup_dup = fds.len() as _;
                        let _ = dup2(c2_dup, c2_dup_dup);
                        let _ = close_range(c2_dup_dup as c::c_uint + 1, !0, 0);
                        let mut pollfd = c::pollfd {
                            fd: c2_dup_dup,
                            events: 0,
                            revents: 0,
                        };
                        let _ = uapi::poll(from_mut(&mut pollfd), -1);
                    }
                    unsafe {
                        c::_exit(0);
                    }
                }
            }
        }
        drop(c1);
        let mut pollfd = c::pollfd {
            fd: p1.raw(),
            events: 0,
            revents: 0,
        };
        let _ = uapi::poll(from_mut(&mut pollfd), -1);
        Some(p2)
    }
}

impl JayReexecRequestHandler for JayReexec {
    type Error = JayReexecError;

    fn arg(&self, req: Arg<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.args.borrow_mut().push(req.arg.to_owned());
        Ok(())
    }

    fn exec(&self, req: Exec<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let args = self.args.borrow();
        let mut args2 = UstrPtr::new();
        args2.push(req.path);
        for arg in &*args {
            args2.push(&**arg);
        }
        let _drop_after_exec = self.delay_close_input_fd();
        if let Err(e) = uapi::execvp(req.path, &args2) {
            self.send_failed(&OsError(e.0).to_string());
        }
        Ok(())
    }
}

object_base! {
    self = JayReexec;
    version = self.version;
}

impl Object for JayReexec {}

simple_add_obj!(JayReexec);

#[derive(Debug, Error)]
pub enum JayReexecError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayReexecError, ClientError);
