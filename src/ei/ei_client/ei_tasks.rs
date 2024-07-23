use {
    crate::{
        async_engine::Phase,
        ei::{
            ei_client::{ei_error::EiClientError, EiClient},
            ei_object::EiObjectId,
        },
        utils::{
            buffd::{BufFdIn, BufFdOut, EiMsgParser},
            errorfmt::ErrorFmt,
            vec_ext::VecExt,
        },
    },
    futures_util::{select, FutureExt},
    std::{collections::VecDeque, mem, rc::Rc, time::Duration},
};

pub async fn ei_client(data: Rc<EiClient>) {
    let mut recv = data.state.eng.spawn(receive(data.clone())).fuse();
    let mut shutdown = data.shutdown.triggered().fuse();
    let _send = data.state.eng.spawn2(Phase::PostLayout, send(data.clone()));
    select! {
        _ = recv => { },
        _ = shutdown => { },
    }
    drop(recv);
    data.flush_request.trigger();
    match data.state.wheel.timeout(5000).await {
        Ok(_) => {
            log::error!("Could not shut down client {} within 5 seconds", data.id);
        }
        Err(e) => {
            log::error!("Could not create a timeout: {}", ErrorFmt(e));
        }
    }
    data.state.ei_clients.kill(data.id);
}

async fn receive(data: Rc<EiClient>) {
    let recv = async {
        let mut buf = BufFdIn::new(&data.socket, &data.state.ring);
        let mut data_buf = Vec::<u32>::new();
        loop {
            let mut hdr = [0u32; 4];
            buf.read_full(&mut hdr[..]).await?;
            #[cfg(target_endian = "little")]
            let obj_id = (hdr[0] as u64) | ((hdr[1] as u64) << 32);
            #[cfg(target_endian = "big")]
            let obj_id = (hdr[1] as u64) | ((hdr[0] as u64) << 32);
            let obj_id = EiObjectId::from_raw(obj_id);
            let len = hdr[2] as usize;
            let request = hdr[3];
            if len < 16 {
                return Err(EiClientError::MessageSizeTooSmall);
            }
            if len > (u16::MAX as usize) {
                return Err(EiClientError::MessageSizeTooLarge);
            }
            if len % 4 != 0 {
                return Err(EiClientError::UnalignedMessage);
            }
            let len = (len - 16) / 4;
            data_buf.clear();
            data_buf.reserve(len);
            let unused = data_buf.split_at_spare_mut_ext().1;
            buf.read_full(&mut unused[..len]).await?;
            unsafe {
                data_buf.set_len(len);
            }
            let obj = match data.objects.get_obj(obj_id) {
                Some(obj) => obj,
                _ => match data.connection.get() {
                    None => {
                        return Err(EiClientError::InvalidObject(obj_id));
                    }
                    Some(c) => {
                        c.send_invalid_object(obj_id);
                        continue;
                    }
                },
            };
            let parser = EiMsgParser::new(&mut buf, &data_buf[..]);
            if let Err(e) = obj.handle_request(&data, request, parser) {
                return Err(EiClientError::RequestError(Box::new(e)));
            }
        }
    };
    let res: Result<(), EiClientError> = recv.await;
    if let Err(e) = res {
        if data.disconnect_announced.get() {
            log::info!("Client {} terminated the connection", data.id);
            data.state.ei_clients.kill(data.id);
        } else {
            let e = ErrorFmt(e);
            log::error!(
                "An error occurred while trying to handle a message from client {}: {}",
                data.id,
                e
            );
            if let Some(c) = data.connection.get() {
                c.send_disconnected(Some(&e.to_string()));
                data.state.ei_clients.shutdown(data.id);
            } else {
                data.state.ei_clients.kill(data.id);
            }
        }
    }
}

async fn send(data: Rc<EiClient>) {
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
    let res: Result<(), EiClientError> = send.await;
    if let Err(e) = res {
        if data.disconnect_announced.get() {
            log::info!("Client {} terminated the connection", data.id);
        } else {
            log::error!(
                "An error occurred while sending data to client {}: {}",
                data.id,
                ErrorFmt(e)
            );
        }
    }
    data.state.ei_clients.kill(data.id);
}
