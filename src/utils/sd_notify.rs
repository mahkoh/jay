use std::{
    env,
    ffi::{OsStr, OsString},
    io,
    os::{
        linux::net::SocketAddrExt,
        unix::{
            ffi::OsStrExt,
            net::{SocketAddr, UnixDatagram},
        },
    },
    path::Path,
};

const NOTIFY_SOCKET: &str = "NOTIFY_SOCKET";

pub fn send_sd_notify(msg: &[u8], path: &OsStr) {
    if let Err(e) = try_send_sd_notify(msg, path) {
        // TODO: This could be logging, but is eprintln! right now, while things are racy.
        eprintln!(
            "Failed to send systemd notification `{}`: {e}",
            String::from_utf8_lossy(msg)
        );
    }
}

fn try_send_sd_notify(msg: &[u8], path: &OsStr) -> io::Result<()> {
    let addr = match path.as_bytes().strip_prefix(b"@") {
        Some(abs) => SocketAddr::from_abstract_name(abs)?,
        None => SocketAddr::from_pathname(Path::new(&path))?,
    };
    let sock = UnixDatagram::unbound()?;
    sock.send_to_addr(msg, &addr)?;

    Ok(())
}

pub fn get_notify_socket() -> Option<OsString> {
    env::var_os(NOTIFY_SOCKET)
}

pub unsafe fn take_notify_socket() -> Option<OsString> {
    let notify_socket = get_notify_socket()?;
    unsafe {
        env::remove_var(NOTIFY_SOCKET);
    }
    Some(notify_socket)
}
