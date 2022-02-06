use std::collections::VecDeque;
use crate::client::{Client, ClientError};
use crate::object::ObjectId;
use crate::utils::buffd::{BufFdIn, BufFdOut, MsgParser};
use crate::utils::oneshot::OneshotRx;
use crate::utils::vec_ext::VecExt;
use crate::ErrorFmt;
use futures::{select, FutureExt};
use std::mem;
use std::rc::Rc;

pub async fn client(data: Rc<Client>, shutdown: OneshotRx<()>) {
    let mut recv = data.state.eng.spawn(receive(data.clone())).fuse();
    let mut dispatch_fr = data.state.eng.spawn(dispatch_fr(data.clone())).fuse();
    let _send = data.state.eng.spawn(send(data.clone()));
    select! {
        _ = recv => { },
        _ = dispatch_fr => { },
        _ = shutdown.fuse() => { },
    }
    drop(recv);
    drop(dispatch_fr);
    data.flush_request.trigger();
    match data.state.eng.timeout(5000) {
        Ok(timeout) => {
            timeout.await;
            log::error!("Could not shut down client {} within 5 seconds", data.id.0);
        }
        Err(e) => {
            log::error!("Could not create a timeout: {}", ErrorFmt(e));
        }
    }
    data.state.clients.kill(data.id);
}

async fn dispatch_fr(data: Rc<Client>) {
    loop {
        let mut fr = data.dispatch_frame_requests.pop().await;
        loop {
            fr.send_done();
            if let Err(e) = data.remove_obj(&*fr) {
                log::error!("Could not remove frame object: {}", ErrorFmt(e));
                return;
            }
            fr = match data.dispatch_frame_requests.try_pop() {
                Some(f) => f,
                _ => break,
            };
        }
        data.flush();
    }
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
                    display.send_invalid_object(obj_id);
                    data.state.clients.shutdown(data.id);
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
            if let Err(e) = obj.handle_request(request, parser) {
                return Err(ClientError::RequestError(Box::new(e)));
            }
            data.flush();
        }
    };
    let res: Result<(), ClientError> = recv.await;
    if let Err(e) = res {
        if e.peer_closed() {
            log::info!("Client {} terminated the connection", data.id.0);
            data.state.clients.kill(data.id);
        } else {
            let e = ErrorFmt(e);
            log::error!(
                "An error occurred while trying to handle a message from client {}: {}",
                data.id.0,
                e
            );
            display.send_implementation_error(e.to_string());
            data.state.clients.shutdown(data.id);
        }
    }
}

async fn send(data: Rc<Client>) {
    let send = async {
        let mut out = BufFdOut::new(data.socket.clone());
        let mut buffers = VecDeque::new();
        loop {
            data.flush_request.triggered().await;
            {
                let mut swapchain = data.swapchain.borrow_mut();
                swapchain.commit();
                mem::swap(&mut swapchain.pending, &mut buffers);
            }
            let mut timeout = None;
            while let Some(mut cur) = buffers.pop_front() {
                out.flush(&mut cur, &mut timeout).await?;
                data.swapchain.borrow_mut().free.push(cur);
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
