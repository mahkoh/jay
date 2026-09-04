use crate::async_engine::Phase;
use crate::client::Client;
use crate::client::ClientError;
use crate::client::Synthetic;
use crate::utils::buffd::BufFdOut;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::SyntheticBufOut;
use crate::utils::buffd::WlBufFdIn;
use crate::utils::buffd::WlMessage;
use crate::utils::errorfmt::ErrorFmt;
use crate::wire::ObjectId;
use futures_util::FutureExt;
use futures_util::select;
use isnt::std_1::primitive::IsntSliceExt;
use run_on_drop::on_drop;
use std::collections::VecDeque;
use std::error::Error;
use std::hint::cold_path;
use std::mem;
use std::rc::Rc;
use std::time::Duration;
use uapi::c;

pub async fn client(data: Rc<Client>) {
    let state = &data.state;
    let eng = &state.eng;
    let mut recv = eng.spawn("client receive", receive(data.clone())).fuse();
    let mut shutdown = data.shutdown.triggered().fuse();
    let _send = eng.spawn2("client send", Phase::PostLayout, send(data.clone()));
    let _synthetic_requests = eng.spawn("synthetic requests", synthetic_requests(data.clone()));
    let _synthetic_events = eng.spawn("synthetic events", synthetic_events(data.clone()));
    let _terminate = eng.spawn("client terminate", terminate(data.clone()));
    select! {
        _ = recv => { },
        _ = shutdown => { },
    }
    drop(recv);
    data.flush_request.trigger();
    match state.wheel.timeout(5000).await {
        Ok(_) => {
            log::error!("Could not shut down client {} within 5 seconds", data.id.0);
        }
        Err(e) => {
            log::error!("Could not create a timeout: {}", ErrorFmt(e));
        }
    }
    state.clients.kill(data.id);
}

async fn terminate(data: Rc<Client>) {
    loop {
        if data.terminate_kill.get() {
            data.state.clients.kill(data.id);
            return;
        }
        if data.terminate_shutdown.get() {
            data.state.clients.shutdown(data.id);
        }
        data.terminate.triggered().await;
    }
}

async fn receive(data: Rc<Client>) {
    let _shutdown_rd = on_drop(|| {
        let _ = uapi::shutdown(data.socket.raw(), c::SHUT_RD);
    });
    let display = data.display().unwrap();
    let blockers = &data.request_blockers;
    let recv = async {
        let mut buf = WlBufFdIn::new(&data.socket, &data.state.ring);
        loop {
            let WlMessage {
                obj_id,
                message,
                body,
                fds,
            } = buf.read_message().await?;
            while blockers.get() > 0 {
                cold_path();
                data.requests_unblocked.triggered().await;
            }
            let obj = match data.objects.get_obj(obj_id) {
                Ok(obj) => obj,
                _ => {
                    display.send_invalid_object(obj_id);
                    data.shutdown();
                    return Err(ClientError::InvalidObject(obj_id));
                }
            };
            let parser = MsgParser::new(fds, body);
            if let Err(e) = obj.handle_request(&data, message, parser) {
                if let ClientError::InvalidMethod = e
                    && let Ok(obj) = data.objects.get_obj(obj_id)
                {
                    data.invalid_request(&*obj, message);
                    return Err(e);
                }
                return Err(ClientError::RequestError(Box::new(e)));
            }
            // data.flush();
        }
    };
    let res: Result<(), ClientError> = recv.await;
    if let Err(e) = res {
        if e.peer_closed() {
            log::info!("Client {} terminated the connection", data.id.0);
            data.kill();
        } else {
            let e = ErrorFmt(e);
            log::error!(
                "An error occurred while trying to handle a message from client {}: {}",
                data.id.0,
                e
            );
            display.send_implementation_error(e.to_string());
            data.shutdown();
        }
    }
}

async fn send(data: Rc<Client>) {
    let socket = data.socket.clone();
    let _shutdown_wr = on_drop(|| {
        let _ = uapi::shutdown(socket.raw(), c::SHUT_WR);
    });
    let send = async {
        let mut out = BufFdOut::new(&data.socket, &data.state.ring);
        let mut buffers = VecDeque::new();
        loop {
            data.flush_request.triggered().await;
            {
                let mut swapchain = data.swapchain.borrow_mut();
                swapchain.commit();
                mem::swap(&mut swapchain.pending, &mut buffers);
            }
            let timeout = data.state.now() + Duration::from_millis(5000);
            while let Some(mut cur) = buffers.pop_front() {
                out.flush(&mut cur, timeout).await?;
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
                "An error occurred while sending data to client {}: {}",
                data.id.0,
                ErrorFmt(e)
            );
        }
    }
    data.kill();
}

async fn synthetic_requests(data: Rc<Client>) {
    synthetic_messages(
        &data,
        "requests",
        &data.synthetic_requests,
        |id| data.objects.get_obj(id).ok(),
        |h, _id, msg, parser| h.handle_request(&data, msg, parser),
    )
    .await;
}

async fn synthetic_events(data: Rc<Client>) {
    synthetic_messages(
        &data,
        "events",
        &data.synthetic_events,
        |id| data.objects.get_synthetic_event_handler(id),
        |h, id, msg, parser| h.handle_event(id, msg, parser),
    )
    .await;
}

async fn synthetic_messages<E, H>(
    data: &Rc<Client>,
    ty: &str,
    messages: &Synthetic,
    get: impl Fn(ObjectId) -> Option<Rc<H>>,
    handle: impl Fn(Rc<H>, ObjectId, u32, MsgParser<'_, '_>) -> Result<(), E>,
) where
    E: Error,
    H: ?Sized,
{
    let handle = async {
        let mut buf_out = SyntheticBufOut::default();
        let mut to_remove = Vec::new();
        let mut unblocked = 0;
        loop {
            if data.request_blockers.sub_fetch(unblocked) == 0 {
                data.requests_unblocked.trigger();
            }
            unblocked = messages.trigger.triggered().await;
            mem::swap(&mut *messages.buf.borrow_mut(), &mut buf_out);
            messages.to_remove.swap(&mut to_remove);
            let (mut buf, fds) = buf_out.take();
            while buf.is_not_empty() {
                let header;
                (header, buf) = buf.split_first_chunk::<4>().unwrap();
                let num_fds = header[0] as usize;
                let obj_hi = header[1] as u64;
                let obj_lo = header[2] as u64;
                let obj = ObjectId::from_raw((obj_hi << 32) | obj_lo);
                let len = (header[3] >> 16) as usize;
                let msg = header[3] & 0xffff;
                let body;
                (body, buf) = buf.split_at(len / 4 - 2);
                let Some(handler) = get(obj) else {
                    if num_fds > 0 {
                        cold_path();
                        fds.drain(..num_fds);
                    }
                    continue;
                };
                let mut parser = MsgParser::new(fds, body);
                parser.wide = true;
                handle(handler, obj, msg, parser)?;
            }
            for id in to_remove.drain(..) {
                data.objects.remove_synthetic_event_handler(id);
            }
        }
    };
    let res: Result<(), E> = handle.await;
    if let Err(e) = res {
        log::error!(
            "An error occurred while handling synthetic {ty} for client {}: {}",
            data.id.0,
            ErrorFmt(e),
        );
    }
    data.kill();
}
