use crate::dbus::{DbusMessage, DbusSocket};
use crate::utils::vec_ext::{UninitVecExt, VecExt};
use crate::utils::vecstorage::VecStorage;
use crate::ErrorFmt;
use std::collections::VecDeque;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::rc::Rc;
use uapi::{c, Errno, Msghdr};

pub async fn handle_outgoing(socket: Rc<DbusSocket>) {
    let mut outgoing = Outgoing {
        socket,
        msgs: Default::default(),
        cmsg: vec![],
        fds: vec![],
        iovecs: Default::default(),
    };
    outgoing.run().await
}

struct DbusMessageOffset {
    msg: DbusMessage,
    offset: usize,
}

struct Outgoing {
    socket: Rc<DbusSocket>,

    msgs: VecDeque<DbusMessageOffset>,
    cmsg: Vec<MaybeUninit<u8>>,
    fds: Vec<c::c_int>,
    iovecs: VecStorage<NonNull<[u8]>>,
}

impl Outgoing {
    async fn run(&mut self) {
        loop {
            self.socket.outgoing.non_empty().await;
            while let Err(e) = self.try_flush() {
                if e != Errno(c::EAGAIN) {
                    log::error!(
                        "{}: Could not send a message to the bus: {}",
                        self.socket.bus_name,
                        ErrorFmt(e)
                    );
                    self.socket.kill();
                    return;
                }
                let _ = self.socket.fd.writable().await;
            }
        }
    }

    fn try_flush(&mut self) -> Result<(), Errno> {
        loop {
            while let Some(msg) = self.socket.outgoing.try_pop() {
                self.msgs.push_back(DbusMessageOffset { msg, offset: 0 });
            }
            if self.msgs.is_empty() {
                return Ok(());
            }
            let mut iovecs = self.iovecs.take_as();
            let mut fds = &[][..];
            for msg in &mut self.msgs {
                if msg.msg.fds.len() > 0 {
                    if fds.len() > 0 || iovecs.len() > 0 {
                        break;
                    }
                    fds = &msg.msg.fds;
                }
                iovecs.push(&msg.msg.buf[msg.offset..]);
            }
            self.cmsg.clear();
            if fds.len() > 0 {
                self.fds.clear();
                self.fds.extend(fds.iter().map(|f| f.raw()));
                let cmsg_space = uapi::cmsg_space(fds.len() * mem::size_of::<c::c_int>());
                self.cmsg.reserve(cmsg_space);
                let (_, mut spare) = self.cmsg.split_at_spare_mut_bytes_ext();
                let hdr = c::cmsghdr {
                    cmsg_len: 0,
                    cmsg_level: c::SOL_SOCKET,
                    cmsg_type: c::SCM_RIGHTS,
                };
                let len = uapi::cmsg_write(&mut spare, hdr, &self.fds).unwrap();
                self.cmsg.set_len_safe(len);
            }
            let msg = Msghdr {
                iov: &iovecs[..],
                control: Some(&self.cmsg[..]),
                name: uapi::sockaddr_none_ref(),
            };
            let mut n = uapi::sendmsg(self.socket.fd.raw(), &msg, c::MSG_DONTWAIT)?;
            drop(iovecs);
            self.msgs[0].msg.fds.clear();
            while n > 0 {
                let len = self.msgs[0].msg.buf.len() - self.msgs[0].offset;
                if n < len {
                    self.msgs[0].offset += n;
                    break;
                }
                n -= len;
                let msg = self.msgs.pop_front().unwrap();
                self.socket.bufs.push(msg.msg.buf);
            }
        }
    }
}
