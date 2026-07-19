use crate::dbus::BUS_DEST;
use crate::dbus::BUS_PATH;
use crate::dbus::DbusError;
use crate::dbus::DictEntry;
use crate::dbus::DynamicType;
use crate::dbus::prelude::Variant;
use crate::state::State;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::opaque::opaque;
use crate::utils::sleeper::Sleeper;
use crate::wire_dbus::org;
use std::borrow::Cow;
use std::rc::Rc;
use thiserror::Error;

const SYSTEMD_DEST: &str = "org.freedesktop.systemd1";
const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";

#[derive(Debug, Error)]
pub enum UserSessionError {
    #[error("Could not access the user session bus")]
    AcquireSessionBus(#[source] DbusError),
}

pub async fn import_environment(state: &Rc<State>, key: &str, value: &str) {
    if let Err(e) = import_environment_(state, key, value).await {
        log::error!(
            "Could not import `{}={}` into the system environment: {}",
            key,
            value,
            ErrorFmt(e)
        );
    }
}

async fn import_environment_(
    state: &Rc<State>,
    key: &str,
    value: &str,
) -> Result<(), UserSessionError> {
    let session = match state.dbus.session().await {
        Ok(s) => s,
        Err(e) => return Err(UserSessionError::AcquireSessionBus(e)),
    };
    let setting = format!("{}={}", key, value);
    session.call(
        BUS_DEST,
        BUS_PATH,
        org::freedesktop::dbus::UpdateActivationEnvironment {
            environment: Cow::Borrowed(&[DictEntry {
                key: key.into(),
                value: value.into(),
            }]),
        },
        {
            let setting = setting.clone();
            move |rep| {
                if let Err(e) = rep {
                    log::error!(
                        "Could not import `{}` into the dbus environment: {}",
                        setting,
                        ErrorFmt(e)
                    );
                }
            }
        },
    );
    session.call(
        SYSTEMD_DEST,
        SYSTEMD_PATH,
        org::freedesktop::systemd1::manager::SetEnvironment {
            names: Cow::Borrowed(&[Cow::Borrowed(&setting)]),
        },
        {
            let setting = setting.clone();
            move |rep| {
                if let Err(e) = rep {
                    log::error!(
                        "Could not import `{}` into the systemd environment: {}",
                        setting,
                        ErrorFmt(e)
                    );
                }
            }
        },
    );
    Ok(())
}

pub async fn start_graphical_session(state: &Rc<State>) {
    let Some(sleeper) = &state.sleeper else {
        log::warn!("Cannot start graphical session because there is no sleeper");
        return;
    };
    if let Err(e) = start_graphical_session_(state, sleeper).await {
        log::error!("Could not start graphical session unit: {}", ErrorFmt(e));
    }
}

async fn start_graphical_session_(state: &Rc<State>, sleeper: &Sleeper) -> Result<(), DbusError> {
    let session = state.dbus.session().await?;
    let name = format!("jay-{}.scope", opaque());
    let properties = [
        (
            Cow::Borrowed("Upholds"),
            Variant::Array(
                DynamicType::String,
                vec![Variant::String(Cow::Borrowed("graphical-session.target"))],
            ),
        ),
        (
            Cow::Borrowed("PIDFDs"),
            Variant::Array(DynamicType::Fd, vec![Variant::Fd(sleeper.pidfd.clone())]),
        ),
    ];
    session
        .call_async(
            SYSTEMD_DEST,
            SYSTEMD_PATH,
            org::freedesktop::systemd1::manager::StartTransientUnit {
                name: Cow::Borrowed(&name),
                mode: Cow::Borrowed("fail"),
                properties: Cow::Borrowed(&properties),
                aux: Default::default(),
            },
        )
        .await?;
    Ok(())
}
