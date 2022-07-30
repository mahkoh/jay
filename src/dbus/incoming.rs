use {
    super::{
        HDR_DESTINATION, HDR_ERROR_NAME, HDR_INTERFACE, HDR_MEMBER, HDR_PATH, HDR_REPLY_SERIAL,
        HDR_SENDER, HDR_SIGNATURE, HDR_UNIX_FDS,
    },
    crate::{
        dbus::{
            CallError, DbusError, DbusSocket, Headers, MemberHandlerKey, Message, MethodHandlerApi,
            Parser, PropertyGetAllHandlerProxy, PropertyGetHandlerProxy, MSG_ERROR,
            MSG_METHOD_CALL, MSG_METHOD_RETURN, MSG_SIGNAL, NO_REPLY_EXPECTED,
        },
        utils::{
            bitflags::BitflagsExt,
            bufio::BufIoIncoming,
            errorfmt::ErrorFmt,
            ptr_ext::{MutPtrExt, PtrExt},
        },
        wire_dbus::org::freedesktop::dbus::properties::{Get, GetAll},
    },
    std::{cell::UnsafeCell, ops::Deref, rc::Rc},
};

pub async fn handle_incoming(socket: Rc<DbusSocket>) {
    let mut incoming = Incoming {
        incoming: socket.bufio.incoming(),
        socket,
    };
    incoming.run().await;
}

pub struct Incoming {
    socket: Rc<DbusSocket>,
    incoming: BufIoIncoming,
}

impl Incoming {
    async fn run(&mut self) {
        loop {
            if self.socket.dead.get() {
                return;
            }
            if let Err(e) = self.handle_msg().await {
                log::error!(
                    "{}: Could not process an incoming message: {}",
                    self.socket.bus_name,
                    ErrorFmt(e)
                );
                self.socket.kill();
                return;
            }
        }
    }

    async fn handle_msg(&mut self) -> Result<(), DbusError> {
        let msg_buf_data = UnsafeCell::new(self.socket.bufio.buf());
        let msg_buf = unsafe { msg_buf_data.get().deref_mut() };
        const FIXED_HEADER_SIZE: usize = 16;
        self.incoming
            .fill_msg_buf(FIXED_HEADER_SIZE, msg_buf)
            .await?;
        let endianess = msg_buf[0];
        if (endianess == b'l') != cfg!(target_endian = "little") {
            return Err(DbusError::InvalidEndianess);
        }
        let msg_ty = msg_buf[1];
        let flags = msg_buf[2];
        let protocol = msg_buf[3];
        if protocol != 1 {
            return Err(DbusError::InvalidProtocol);
        }
        let mut fields2 = [0u32; 3];
        uapi::pod_write(&msg_buf[4..], &mut fields2[..]).unwrap();
        let [body_len, serial, headers_len] = fields2;
        let dyn_header_len = headers_len + (headers_len.wrapping_neg() & 7);
        let remaining = dyn_header_len + body_len;
        self.incoming
            .fill_msg_buf(remaining as usize, msg_buf)
            .await?;
        #[allow(clippy::drop_ref)]
        drop(msg_buf);
        let msg_buf = unsafe { msg_buf_data.get().deref().deref() };
        let headers = &msg_buf[FIXED_HEADER_SIZE..FIXED_HEADER_SIZE + headers_len as usize];
        let headers = self.parse_headers(headers)?;
        let unix_fds = headers.unix_fds.unwrap_or(0) as usize;
        if self.incoming.fds.len() < unix_fds {
            return Err(DbusError::TooFewFds);
        }
        let fds: Vec<_> = self.incoming.fds.drain(..unix_fds).collect();
        let mut parser = Parser {
            buf: msg_buf,
            pos: FIXED_HEADER_SIZE + dyn_header_len as usize,
            fds: &fds,
        };
        match msg_ty {
            MSG_METHOD_CALL => {
                let (sender, interface, member, path) = match (
                    &headers.sender,
                    &headers.interface,
                    &headers.member,
                    &headers.path,
                ) {
                    (Some(s), Some(i), Some(m), Some(p)) => (s, i, m, p),
                    _ => return Err(DbusError::MissingMethodCallHeaders),
                };
                if let Some(object) = self.socket.objects.get(path.deref()) {
                    let method_handler;
                    let handler: Option<&dyn MethodHandlerApi> =
                        if (interface.deref(), member.deref()) == (Get::INTERFACE, Get::MEMBER) {
                            Some(&PropertyGetHandlerProxy)
                        } else if (interface.deref(), member.deref())
                            == (GetAll::INTERFACE, GetAll::MEMBER)
                        {
                            Some(&PropertyGetAllHandlerProxy)
                        } else {
                            let key = MemberHandlerKey {
                                interface: interface.deref(),
                                member: member.deref(),
                            };
                            method_handler = object.methods.get(&key);
                            method_handler.as_ref().map(|mh| mh.deref())
                        };
                    if let Some(handler) = handler {
                        let sig = headers.signature.as_deref().unwrap_or("");
                        if sig != handler.signature() {
                            let msg = format!(
                                "Method call has an invalid signature: expected: {}, actual: {}",
                                handler.signature(),
                                sig,
                            );
                            self.socket.send_error(sender.deref(), serial, &msg);
                        } else {
                            let reply_expected = !flags.contains(NO_REPLY_EXPECTED);
                            if let Err(e) = handler.handle(
                                &object,
                                &self.socket,
                                &sender,
                                serial,
                                reply_expected,
                                &mut parser,
                            ) {
                                log::error!(
                                    "{}: Could not handle method call: {}",
                                    self.socket.bus_name,
                                    ErrorFmt(e)
                                );
                            }
                        }
                    } else {
                        self.socket
                            .send_error(sender.deref(), serial, "Method does not exist");
                    }
                } else {
                    self.socket
                        .send_error(sender.deref(), serial, "Object does not exist");
                }
            }
            MSG_METHOD_RETURN | MSG_ERROR => {
                let serial = match headers.reply_serial {
                    Some(s) => s,
                    _ => return Err(DbusError::NoReplySerial),
                };
                if let Some(reply) = self.socket.reply_handlers.remove(&serial) {
                    if msg_ty == MSG_ERROR {
                        let ename = match headers.error_name {
                            Some(n) => n.into_owned(),
                            _ => return Err(DbusError::NoErrorName),
                        };
                        let mut emsg = None;
                        if let Some(sig) = headers.signature {
                            if sig.0.starts_with("s") {
                                emsg = Some(parser.read_string()?.into_owned());
                            }
                        }
                        let error = CallError {
                            name: ename,
                            msg: emsg,
                        };
                        reply.handle_error(&self.socket, DbusError::CallError(error));
                    } else {
                        let sig = headers.signature.as_deref().unwrap_or("");
                        if sig != reply.signature() {
                            log::error!(
                                "{}: Message reply has an invalid signature: expected: {}, actual: {}",
                                self.socket.bus_name,
                                reply.signature(),
                                sig,
                            );
                        } else {
                            let buf = unsafe { std::mem::take(msg_buf_data.get().deref_mut()) };
                            if let Err(e) = reply.handle(&self.socket, &headers, &mut parser, buf) {
                                log::error!(
                                    "{}: Could not handle reply: {}",
                                    self.socket.bus_name,
                                    ErrorFmt(e)
                                );
                            }
                        }
                    }
                }
            }
            MSG_SIGNAL => {
                let (interface, member, path) =
                    match (&headers.interface, &headers.member, &headers.path) {
                        (Some(i), Some(m), Some(p)) => (i, m, p),
                        _ => return Err(DbusError::MissingSignalHeaders),
                    };
                let handlers = self.socket.signal_handlers.borrow_mut();
                if let Some(handler) = handlers.get(&(interface.deref(), member.deref())) {
                    let handler = handler
                        .conditional
                        .get(path.deref())
                        .or(handler.unconditional.as_ref());
                    if let Some(handler) = handler {
                        let sig = headers.signature.as_deref().unwrap_or("");
                        if sig != handler.signature() {
                            log::error!(
                                "{}: Signal has an invalid signature: expected: {}, actual: {}",
                                self.socket.bus_name,
                                handler.signature(),
                                sig,
                            );
                        } else {
                            if let Err(e) = handler.handle(&mut parser) {
                                log::error!(
                                    "{}: Could not handle signal: {}",
                                    self.socket.bus_name,
                                    ErrorFmt(e)
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        let msg_buf = msg_buf_data.into_inner();
        if msg_buf.capacity() > 0 {
            self.socket.bufio.add_buf(msg_buf);
        }
        Ok(())
    }

    fn parse_headers<'a>(&self, buf: &'a [u8]) -> Result<Headers<'a>, DbusError> {
        let mut parser = Parser::new(buf, &[]);
        let mut headers = Headers::default();
        while !parser.eof() {
            parser.align_to(8)?;
            let ty: u8 = parser.read_pod()?;
            let val = parser.read_variant()?;
            match ty {
                HDR_PATH => headers.path = Some(val.into_object_path()?),
                HDR_INTERFACE => headers.interface = Some(val.into_string()?),
                HDR_MEMBER => headers.member = Some(val.into_string()?),
                HDR_ERROR_NAME => headers.error_name = Some(val.into_string()?),
                HDR_REPLY_SERIAL => headers.reply_serial = Some(val.into_u32()?),
                HDR_DESTINATION => headers.destination = Some(val.into_string()?),
                HDR_SENDER => headers.sender = Some(val.into_string()?),
                HDR_SIGNATURE => headers.signature = Some(val.into_signature()?),
                HDR_UNIX_FDS => headers.unix_fds = Some(val.into_u32()?),
                _ => {}
            }
        }
        Ok(headers)
    }
}
