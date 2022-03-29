use crate::dbus::DbusSocket;
use crate::utils::errorfmt::ErrorFmt;
use std::rc::Rc;

pub async fn handle_outgoing(socket: Rc<DbusSocket>) {
    if let Err(e) = socket.bufio.clone().outgoing().await {
        log::error!("{}: {}", socket.bus_name, ErrorFmt(e));
    }
    socket.kill();
}
