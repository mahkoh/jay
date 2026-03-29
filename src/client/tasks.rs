use {
    crate::{
        async_engine::Phase,
        client::{Client, ClientError},
        utils::{
            buffd::{BufFdOut, MsgParser, WlBufFdIn, WlMessage},
            errorfmt::ErrorFmt,
        },
    },
    futures_util::{FutureExt, select},
    run_on_drop::on_drop,
    std::{collections::VecDeque, mem, rc::Rc, time::Duration},
    uapi::c,
};

pub async fn client(data: Rc<Client>) {
    let mut recv = data
        .state
        .eng
        .spawn("client receive", receive(data.clone()))
        .fuse();
    let mut shutdown = data.shutdown.triggered().fuse();
    let _send = data
        .state
        .eng
        .spawn2("client send", Phase::PostLayout, send(data.clone()));
    select! {
        _ = recv => { },
        _ = shutdown => { },
    }
    drop(recv);
    data.flush_request.trigger();
    match data.state.wheel.timeout(5000).await {
        Ok(_) => {
            log::error!("Could not shut down client {} within 5 seconds", data.id.0);
        }
        Err(e) => {
            log::error!("Could not create a timeout: {}", ErrorFmt(e));
        }
    }
    data.state.clients.kill(data.id);
}

async fn receive(data: Rc<Client>) {
    let _shutdown_rd = on_drop(|| {
        let _ = uapi::shutdown(data.socket.raw(), c::SHUT_RD);
    });
    let display = data.display().unwrap();
    let recv = async {
        let mut buf = WlBufFdIn::new(&data.socket, &data.state.ring);
        loop {
            let WlMessage {
                obj_id,
                message,
                body,
                fds,
            } = buf.read_message().await?;
            let obj = match data.objects.get_obj(obj_id) {
                Ok(obj) => obj,
                _ => {
                    display.send_invalid_object(obj_id);
                    data.state.clients.shutdown(data.id);
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
    let run_toplevel = data.state.run_toplevel.clone();
    run_toplevel.schedule(move || {
        data.state.clients.kill(data.id);
    });
}
