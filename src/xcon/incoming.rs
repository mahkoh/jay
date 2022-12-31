use {
    crate::{
        utils::{bufio::BufIoIncoming, errorfmt::ErrorFmt},
        xcon::{
            consts::XGE_EVENT, Event, ExtensionData, ExtensionIdRange, Parser, XconData, XconError,
        },
    },
    std::{mem, rc::Rc},
};

pub(super) async fn handle_incoming(xorg: Rc<XconData>, incoming: BufIoIncoming) {
    let mut incoming = Incoming {
        incoming,
        socket: xorg,
        ed: None,
    };
    incoming.run().await;
}

pub struct Incoming {
    socket: Rc<XconData>,
    incoming: BufIoIncoming,
    ed: Option<Rc<ExtensionData>>,
}

impl Incoming {
    async fn run(&mut self) {
        loop {
            if self.socket.dead.get() {
                return;
            }
            if let Err(e) = self.handle_msg().await {
                log::error!("Could not process an incoming message: {}", ErrorFmt(e));
                self.socket.kill();
                return;
            }
        }
    }

    #[allow(clippy::await_holding_refcell_ref)] // false positive
    async fn handle_msg(&mut self) -> Result<(), XconError> {
        const MAX_LENGTH_UNITS: usize = 0x4000 / 4;
        const MIN_MSG_SIZE: usize = 32;

        let mut msg_buf = self.socket.in_bufs.pop().unwrap_or_default();
        msg_buf.clear();
        self.incoming
            .fill_msg_buf(MIN_MSG_SIZE, &mut msg_buf)
            .await?;
        let mut serial = 0;
        const KEYMAP_NOTIFY: u8 = 11;
        let mut reply_handlers = self.socket.reply_handlers.borrow_mut();
        if msg_buf[0] & 0x7f != KEYMAP_NOTIFY {
            let serial_16 = u16::from_ne_bytes([msg_buf[2], msg_buf[3]]);
            serial = (self.socket.last_recv_serial.get() & !0xffff) | (serial_16 as u64);
            if serial < self.socket.last_recv_serial.get() {
                serial += 0x10000;
            }
            self.socket.last_recv_serial.set(serial);
            while let Some(first) = reply_handlers.front() {
                if first.serial() < serial {
                    let handler = reply_handlers.pop_front().unwrap();
                    drop(reply_handlers);
                    handler.handle_noreply(&self.socket)?;
                    reply_handlers = self.socket.reply_handlers.borrow_mut();
                } else {
                    break;
                }
            }
        }
        if self.ed.is_none() {
            self.ed = self.socket.extensions.get();
        }
        match msg_buf[0] & 0x7f {
            0 => 'handle_error: {
                let code = msg_buf[1];
                let (ext, code) = if code < 128 {
                    (None, code)
                } else if let Some(ed) = &self.ed {
                    let r = match find_range(&ed.errors, code) {
                        Some(r) => r,
                        _ => {
                            log::error!("Received an out of bounds error code {}", code);
                            break 'handle_error;
                        }
                    };
                    match r.extension {
                        Some(e) => (Some(e), code - r.first),
                        None => {
                            log::warn!(
                                "Received an error from an unconfigured extension: `{}`",
                                r.name
                            );
                            break 'handle_error;
                        }
                    }
                } else {
                    log::error!("Received an extension error before extension have been fetched");
                    break 'handle_error;
                };
                let e = match ext {
                    Some(e) => XconError::ExtensionError(e, code),
                    _ => XconError::CoreError(code),
                };
                if let Some(first) = reply_handlers.front() {
                    if first.serial() == serial {
                        let handler = reply_handlers.pop_front().unwrap();
                        drop(reply_handlers);
                        handler.handle_error(e);
                        break 'handle_error;
                    }
                }
                log::error!(
                    "Received an error with no corresponding handler: {}",
                    ErrorFmt(e)
                );
            }
            1 => {
                if let Some(first) = reply_handlers.front() {
                    if first.serial() == serial {
                        let handler = reply_handlers.pop_front().unwrap();
                        drop(reply_handlers);
                        let mut fds = vec![];
                        if handler.has_fds() {
                            let num_fds = msg_buf[1] as usize;
                            if self.incoming.fds.len() < num_fds {
                                return Err(XconError::MissingFds);
                            }
                            fds.extend(self.incoming.fds.drain(..num_fds));
                        }
                        let length =
                            u32::from_ne_bytes([msg_buf[4], msg_buf[5], msg_buf[6], msg_buf[7]])
                                as usize;
                        if length > MAX_LENGTH_UNITS {
                            return Err(XconError::ExcessiveMessageSize);
                        }
                        let length = length * 4;
                        self.incoming.fill_msg_buf(length, &mut msg_buf).await?;
                        let mut parser = unsafe {
                            let msg_buf = mem::transmute::<&[u8], &'static [u8]>(&msg_buf[..]);
                            Parser::new(msg_buf, fds)
                        };
                        handler.handle_result(
                            &self.socket,
                            &mut parser,
                            mem::take(&mut msg_buf),
                        )?;
                    }
                }
            }
            ev => 'handle_event: {
                drop(reply_handlers);
                let (ext, code) = if ev == XGE_EVENT {
                    let length =
                        u32::from_ne_bytes([msg_buf[4], msg_buf[5], msg_buf[6], msg_buf[7]])
                            as usize;
                    if length > MAX_LENGTH_UNITS {
                        return Err(XconError::ExcessiveMessageSize);
                    }
                    let length = length * 4;
                    self.incoming.fill_msg_buf(length, &mut msg_buf).await?;
                    let opcode = msg_buf[1];
                    let ext = match &self.ed {
                        Some(ed) => ed.ext_by_opcode.get(&opcode),
                        _ => {
                            log::error!("Received an XGE event before extension have been fetched");
                            break 'handle_event;
                        }
                    };
                    let ext = match ext {
                        Some(ext) => *ext,
                        _ => {
                            log::warn!(
                                "Received an event from an unconfigured extension: `{}`",
                                opcode
                            );
                            break 'handle_event;
                        }
                    };
                    let code = u16::from_ne_bytes([msg_buf[8], msg_buf[9]]);
                    (Some(ext), code)
                } else if ev < 64 {
                    (None, ev as u16)
                } else if let Some(ed) = &self.ed {
                    let r = match find_range(&ed.events, ev) {
                        Some(r) => r,
                        _ => {
                            log::error!("Received an out of bounds event {}", ev);
                            break 'handle_event;
                        }
                    };
                    match r.extension {
                        Some(e) => (Some(e), (ev - r.first) as u16),
                        None => {
                            log::warn!(
                                "Received an event from an unconfigured extension: `{}`",
                                r.name
                            );
                            break 'handle_event;
                        }
                    }
                } else {
                    log::error!("Received an extension event before extension have been fetched");
                    break 'handle_event;
                };
                self.socket.events.push(Event {
                    socket: self.socket.clone(),
                    ext,
                    code,
                    buf: mem::take(&mut msg_buf),
                    serial,
                });
            }
        }
        if msg_buf.capacity() > 0 {
            self.socket.in_bufs.push(msg_buf);
        }
        Ok(())
    }
}

fn find_range(codes: &[ExtensionIdRange], code: u8) -> Option<&ExtensionIdRange> {
    let idx = match codes.binary_search_by_key(&code, |v| v.first) {
        Ok(v) => v,
        Err(v) if v > 0 => v - 1,
        _ => return None,
    };
    Some(&codes[idx])
}
