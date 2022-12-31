use {
    crate::{
        dbus::{DbusError, DictEntry, BUS_DEST, BUS_PATH},
        state::State,
        utils::errorfmt::ErrorFmt,
        wire_dbus::org,
    },
    std::{borrow::Cow, rc::Rc},
    thiserror::Error,
};

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
