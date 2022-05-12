use {
    crate::async_engine::{AsyncEngine, AsyncError, AsyncFd},
    std::{rc::Rc, time::Duration},
    uapi::c,
};

#[derive(Clone)]
pub struct Timer {
    fd: AsyncFd,
}

impl Timer {
    pub(super) fn new(eng: &Rc<AsyncEngine>, clock_id: c::c_int) -> Result<Self, AsyncError> {
        let fd = match uapi::timerfd_create(clock_id, c::TFD_CLOEXEC | c::TFD_NONBLOCK) {
            Ok(fd) => fd,
            Err(e) => return Err(AsyncError::CreateTimer(e.into())),
        };
        let afd = eng.fd(&Rc::new(fd))?;
        Ok(Self { fd: afd })
    }

    pub async fn expired(&self) -> Result<u64, AsyncError> {
        self.fd.readable().await?;
        let mut buf = 0u64;
        if let Err(e) = uapi::read(self.fd.raw(), &mut buf) {
            return Err(AsyncError::TimerReadError(e.into()));
        }
        Ok(buf)
    }

    pub fn program(
        &self,
        initial: Option<Duration>,
        periodic: Option<Duration>,
    ) -> Result<(), AsyncError> {
        let mut timerspec: c::itimerspec = uapi::pod_zeroed();
        if let Some(init) = initial {
            timerspec.it_value.tv_sec = init.as_secs() as _;
            timerspec.it_value.tv_nsec = init.subsec_nanos() as _;
            if let Some(per) = periodic {
                timerspec.it_interval.tv_sec = per.as_secs() as _;
                timerspec.it_interval.tv_nsec = per.subsec_nanos() as _;
            }
        }
        if let Err(e) = uapi::timerfd_settime(self.fd.raw(), 0, &timerspec) {
            return Err(AsyncError::SetTimer(e.into()));
        }
        Ok(())
    }
}
