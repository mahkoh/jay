use crate::dbus::{DbusError, DbusSocket};
use crate::org::freedesktop::login1::session::TakeDeviceReply;
use crate::{org, FALSE};
use std::rc::Rc;
use thiserror::Error;
use uapi::c;

const LOGIND_NAME: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";

#[derive(Debug, Error)]
pub enum LogindError {
    #[error("XDG_SESSION_ID is not set")]
    XdgSessionId,
    #[error("Could not retrieve the session dbus path")]
    GetSession(DbusError),
    #[error("Could not retrieve the session's seat name")]
    GetSeatName(DbusError),
    #[error(transparent)]
    TakeControl(DbusError),
}

pub struct Session {
    socket: Rc<DbusSocket>,
    seat: String,
    session_path: String,
}

impl Session {
    pub async fn get(socket: &Rc<DbusSocket>) -> Result<Self, LogindError> {
        let session_id = match std::env::var("XDG_SESSION_ID") {
            Ok(id) => id,
            _ => return Err(LogindError::XdgSessionId),
        };
        let session_path = {
            let session = socket
                .call_async(
                    LOGIND_NAME,
                    MANAGER_PATH,
                    org::freedesktop::login1::manager::GetSession {
                        session_id: session_id.as_str().into(),
                    },
                )
                .await;
            match session {
                Ok(s) => s.get().object_path.to_string(),
                Err(e) => return Err(LogindError::GetSession(e)),
            }
        };
        let seat = {
            let seat = socket
                .get_async::<org::freedesktop::login1::session::Seat>(LOGIND_NAME, &session_path)
                .await;
            match seat {
                Ok(s) => s.get().0.to_string(),
                Err(e) => return Err(LogindError::GetSeatName(e)),
            }
        };
        Ok(Self {
            socket: socket.clone(),
            seat,
            session_path,
        })
    }

    pub async fn take_control(&self) -> Result<(), LogindError> {
        let res = self
            .socket
            .call_async(
                LOGIND_NAME,
                &self.session_path,
                org::freedesktop::login1::session::TakeControl { force: FALSE },
            )
            .await;
        match res {
            Ok(_) => Ok(()),
            Err(e) => Err(LogindError::TakeControl(e)),
        }
    }

    pub fn get_device<F>(&self, dev: c::dev_t, f: F)
    where
        F: FnOnce(Result<&TakeDeviceReply, DbusError>) + 'static,
    {
        let major = uapi::major(dev) as _;
        let minor = uapi::minor(dev) as _;
        self.socket.call(
            LOGIND_NAME,
            &self.session_path,
            org::freedesktop::login1::session::TakeDevice { major, minor },
            move |r| f(r),
        );
    }
}
