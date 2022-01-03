use crate::client::{Client, ClientError, WlEvent};
use crate::object::ObjectId;
use crate::utils::buffd::{BufFdIn, BufFdOut, MsgFormatter, MsgParser};
use crate::utils::oneshot::OneshotRx;
use crate::utils::vec_ext::VecExt;
use anyhow::anyhow;
use futures::{select, FutureExt};
use std::mem;
use std::rc::Rc;

pub async fn client(data: Rc<Client>, shutdown: OneshotRx<()>) {
    let mut recv = data.state.eng.spawn(receive(data.clone())).fuse();
    let _send = data.state.eng.spawn(send(data.clone()));
    select! {
        _ = recv => { },
        _ = shutdown.fuse() => { },
    }
    drop(recv);
    if !data.shutdown_sent.get() {
        data.events.push(WlEvent::Shutdown);
    }
    match data.state.eng.timeout(5000) {
        Ok(timeout) => {
            timeout.await;
            log::error!("Could not shut down client {} within 5 seconds", data.id.0);
        }
        Err(e) => {
            log::error!("Could not create a timeout: {:#}", e);
        }
    }
    data.state.clients.kill(data.id);
}

async fn receive(data: Rc<Client>) {
    let display = data.display().unwrap();
    let recv = async {
        let mut buf = BufFdIn::new(data.socket.clone());
        let mut data_buf = Vec::<u32>::new();
        loop {
            let mut hdr = [0u32, 0];
            buf.read_full(&mut hdr[..]).await?;
            let obj_id = ObjectId::from_raw(hdr[0]);
            let len = (hdr[1] >> 16) as usize;
            let request = hdr[1] & 0xffff;
            let obj = match data.objects.get_obj(obj_id) {
                Ok(obj) => obj,
                _ => {
                    data.fatal_event(display.invalid_object(obj_id));
                    return Err(ClientError::InvalidObject(obj_id));
                }
            };
            // log::trace!("obj: {}, request: {}, len: {}", obj_id, request, len);
            if request >= obj.num_requests() {
                data.invalid_request(&*obj, request);
                return Err(ClientError::InvalidMethod);
            }
            if len < 8 {
                return Err(ClientError::MessageSizeTooSmall);
            }
            if len % 4 != 0 {
                return Err(ClientError::UnalignedMessage);
            }
            let len = len / 4 - 2;
            data_buf.clear();
            data_buf.reserve(len);
            let unused = data_buf.split_at_spare_mut_ext().1;
            buf.read_full(&mut unused[..len]).await?;
            unsafe {
                data_buf.set_len(len);
            }
            // log::trace!("{:x?}", data_buf);
            let parser = MsgParser::new(&mut buf, &data_buf[..]);
            if let Err(e) = obj.handle_request(request, parser).await {
                return Err(ClientError::RequestError(Box::new(e)));
            }
            data.event2(WlEvent::Flush).await?;
        }
    };
    let res: Result<(), ClientError> = recv.await;
    if let Err(e) = res {
        if e.peer_closed() {
            log::info!("Client {} terminated the connection", data.id.0);
            data.state.clients.kill(data.id);
        } else {
            let e = anyhow!(e);
            log::error!(
                "An error occurred while trying to handle a message from client {}: {:#}",
                data.id.0,
                e
            );
            if !data.shutdown_sent.get() {
                data.fatal_event(display.implementation_error(format!("{:#}", e)));
            }
        }
    }
}

async fn send(data: Rc<Client>) {
    let send = async {
        let mut buf = BufFdOut::new(data.socket.clone());
        let mut flush_requested = false;
        loop {
            let mut event = data.events.pop().await;
            loop {
                match event {
                    WlEvent::Flush => {
                        flush_requested = true;
                    }
                    WlEvent::Shutdown => {
                        buf.flush().await?;
                        return Ok(());
                    }
                    WlEvent::Event(e) => {
                        if log::log_enabled!(log::Level::Trace) {
                            data.log_event(&*e);
                        }
                        let mut fds = vec![];
                        let mut fmt = MsgFormatter::new(&mut buf, &mut fds);
                        e.format(&mut fmt);
                        fmt.write_len();
                        if buf.needs_flush() {
                            buf.flush().await?;
                            flush_requested = false;
                        }
                    }
                }
                event = match data.events.try_pop() {
                    Some(e) => e,
                    _ => break,
                };
            }
            if mem::take(&mut flush_requested) {
                buf.flush().await?;
            }
        }
    };
    let res: Result<(), ClientError> = send.await;
    if let Err(e) = res {
        if e.peer_closed() {
            log::info!("Client {} terminated the connection", data.id.0);
        } else {
            log::error!(
                "An error occurred while sending data to client {}: {:#}",
                data.id.0,
                e
            );
        }
    }
    data.state.clients.kill(data.id);
}
