use std::{
    env,
    ffi::OsStr,
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

pub fn send_sd_notify_if_enabled(msg: &[u8]) {
    match env::var_os("NOTIFY_SOCKET") {
        Some(notify_socket) => {
            if let Err(e) = try_send_sd_notify(msg, notify_socket.as_os_str()) {
                log::error!("Failed to send systemd ready notification: {e}");
            }
        }
        None => {
            log::debug!("Not sending sd notification, NOTIFY_SOCKET not set");
        }
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
