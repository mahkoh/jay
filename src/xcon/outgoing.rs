use crate::xcon::XconData;
use crate::ErrorFmt;
use std::rc::Rc;

pub(super) async fn handle_outgoing(socket: Rc<XconData>) {
    if let Err(e) = socket.bufio.clone().outgoing().await {
        log::error!("{}", ErrorFmt(e));
    }
    socket.kill();
}
