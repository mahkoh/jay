use super::{
    HDR_DESTINATION, HDR_ERROR_NAME, HDR_INTERFACE, HDR_MEMBER, HDR_PATH, HDR_REPLY_SERIAL,
    HDR_SENDER, HDR_SIGNATURE, HDR_UNIX_FDS,
};
use crate::dbus::{
    CallError, DbusError, DbusSocket, Headers, Parser, MSG_ERROR, MSG_METHOD_RETURN, MSG_SIGNAL,
};
use crate::utils::ptr_ext::{MutPtrExt, PtrExt};
use crate::ErrorFmt;
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::rc::Rc;
use uapi::{c, Errno, MaybeUninitSliceExt, MsghdrMut, OwnedFd};

pub async fn handle_incoming(socket: Rc<DbusSocket>) {
    let mut incoming = Incoming {
        socket,
        buf: Box::new([MaybeUninit::uninit(); 4096]),
        buf_start: 0,
        buf_end: 0,
        fds: Default::default(),
        cmsg: Box::new([MaybeUninit::uninit(); 256]),
    };
    incoming.run().await;
}

pub struct Incoming {
    socket: Rc<DbusSocket>,

    buf: Box<[MaybeUninit<u8>; 4096]>,
    buf_start: usize,
    buf_end: usize,
    fds: VecDeque<Rc<OwnedFd>>,
    cmsg: Box<[MaybeUninit<u8>; 256]>,
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
        let msg_buf_data = UnsafeCell::new(self.socket.bufs.pop().unwrap_or_default());
        let msg_buf = unsafe { msg_buf_data.get().deref_mut() };
        msg_buf.clear();
        const FIXED_HEADER_SIZE: usize = 16;
        self.fill_msg_buf(FIXED_HEADER_SIZE, msg_buf).await?;
        let endianess = msg_buf[0];
        if (endianess == b'l') != cfg!(target_endian = "little") {
            return Err(DbusError::InvalidEndianess);
        }
        let msg_ty = msg_buf[1];
        let _flags = msg_buf[2];
        let protocol = msg_buf[3];
        if protocol != 1 {
            return Err(DbusError::InvalidProtocol);
        }
        let mut fields2 = [0u32; 3];
        uapi::pod_write(&msg_buf[4..], &mut fields2[..]).unwrap();
        let [body_len, _serial, headers_len] = fields2;
        let dyn_header_len = headers_len + (headers_len.wrapping_neg() & 7);
        let remaining = dyn_header_len + body_len;
        self.fill_msg_buf(remaining as usize, msg_buf).await?;
        drop(msg_buf);
        let msg_buf = unsafe { msg_buf_data.get().deref().deref() };
        let headers = &msg_buf[FIXED_HEADER_SIZE..FIXED_HEADER_SIZE + headers_len as usize];
        let headers = self.parse_headers(headers)?;
        let unix_fds = headers.unix_fds.unwrap_or(0) as usize;
        if self.fds.len() < unix_fds {
            return Err(DbusError::TooFewFds);
        }
        let fds: Vec<_> = self.fds.drain(..unix_fds).collect();
        let mut parser = Parser {
            buf: &msg_buf,
            pos: FIXED_HEADER_SIZE + dyn_header_len as usize,
            fds: &fds,
        };
        match msg_ty {
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
            self.socket.bufs.push(msg_buf);
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

    async fn fill_msg_buf(&mut self, mut n: usize, buf: &mut Vec<u8>) -> Result<(), DbusError> {
        while n > 0 {
            if self.buf_start == self.buf_end {
                while let Err(e) = self.recvmsg() {
                    if e.0 != c::EAGAIN {
                        return Err(DbusError::ReadError(e.into()));
                    }
                    self.socket.fd.readable().await?;
                }
                if self.buf_start == self.buf_end {
                    return Err(DbusError::Closed);
                }
            }
            let read = n.min(self.buf_end - self.buf_start);
            let buf_start = self.buf_start % self.buf.len();
            unsafe {
                buf.extend_from_slice(
                    self.buf[buf_start..buf_start + read].slice_assume_init_ref(),
                );
            }
            n -= read;
            self.buf_start += read;
        }
        Ok(())
    }

    fn recvmsg(&mut self) -> Result<(), Errno> {
        self.buf_start = 0;
        self.buf_end = 0;
        let mut iov = [&mut self.buf[..]];
        let mut hdr = MsghdrMut {
            iov: &mut iov[..],
            control: Some(&mut self.cmsg[..]),
            name: uapi::sockaddr_none_mut(),
            flags: 0,
        };
        let (ivec, _, mut cmsg) =
            uapi::recvmsg(self.socket.fd.raw(), &mut hdr, c::MSG_CMSG_CLOEXEC)?;
        self.buf_end += ivec.len();
        while cmsg.len() > 0 {
            let (_, hdr, body) = uapi::cmsg_read(&mut cmsg)?;
            if hdr.cmsg_level == c::SOL_SOCKET && hdr.cmsg_type == c::SCM_RIGHTS {
                for fd in uapi::pod_iter(body)? {
                    self.fds.push_back(Rc::new(OwnedFd::new(fd)));
                }
            }
        }
        Ok(())
    }
}
