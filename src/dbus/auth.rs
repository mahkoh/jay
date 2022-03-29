use crate::dbus::incoming::handle_incoming;
use crate::dbus::outgoing::handle_outgoing;
use crate::dbus::{DbusError, DbusSocket};
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::hex;
use std::io::Write;
use std::rc::Rc;
use uapi::{c, Errno};

pub(super) async fn handle_auth(socket: Rc<DbusSocket>) {
    let mut auth = Auth {
        socket: socket.clone(),
        buf: Box::new([0; BUF_SIZE]),
        buf_start: 0,
        buf_stop: 0,
    };
    auth.run().await;
}

const BUF_SIZE: usize = 128;

struct Auth {
    socket: Rc<DbusSocket>,

    buf: Box<[u8; BUF_SIZE]>,
    buf_start: usize,
    buf_stop: usize,
}

impl Auth {
    async fn run(&mut self) {
        if let Err(e) = self.handle_auth().await {
            log::error!(
                "{}: Could not authenticate to dbus socket: {}",
                self.socket.bus_name,
                ErrorFmt(e)
            );
            self.socket.kill();
            return;
        }
        log::info!("{}: Authenticated", self.socket.bus_name);
        self.socket.incoming.set(Some(
            self.socket.eng.spawn(handle_incoming(self.socket.clone())),
        ));
        self.socket.outgoing_.set(Some(
            self.socket.eng.spawn(handle_outgoing(self.socket.clone())),
        ));
        self.socket.auth.take();
    }

    async fn handle_auth(&mut self) -> Result<(), DbusError> {
        let uid = hex::to_hex(&uapi::getuid().to_string());
        let mut out_buf = Vec::new();
        let _ = write!(out_buf, "\0AUTH EXTERNAL {}\r\n", uid);
        self.write_buf(&mut out_buf).await?;
        let line = self.readline().await?;
        let (cmd, _) = line_to_cmd(&line);
        if cmd != "OK" {
            return Err(DbusError::Auth);
        }
        let _ = write!(out_buf, "NEGOTIATE_UNIX_FD\r\n");
        self.write_buf(&mut out_buf).await?;
        let line = self.readline().await?;
        let (cmd, _) = line_to_cmd(&line);
        if cmd != "AGREE_UNIX_FD" {
            return Err(DbusError::UnixFd);
        }
        let _ = write!(out_buf, "BEGIN\r\n");
        self.write_buf(&mut out_buf).await?;
        Ok(())
    }

    async fn readline(&mut self) -> Result<String, DbusError> {
        let mut s = String::new();
        loop {
            for i in self.buf_start..self.buf_stop {
                let c = self.buf[i % BUF_SIZE] as char;
                s.push(c);
                if c == '\n' {
                    self.buf_start = i + 1;
                    return Ok(s);
                }
            }
            self.buf_start = 0;
            self.buf_stop = 0;
            match uapi::read(self.socket.fd.raw(), &mut self.buf[..]) {
                Ok(n) => self.buf_stop = n.len(),
                Err(Errno(c::EAGAIN)) => {
                    self.socket.fd.readable().await?;
                }
                Err(e) => return Err(DbusError::ReadError(e.into())),
            }
        }
    }

    async fn write_buf(&mut self, buf: &mut Vec<u8>) -> Result<(), DbusError> {
        let mut start = 0;
        while start < buf.len() {
            match uapi::write(self.socket.fd.raw(), &buf[start..]) {
                Ok(n) => start += n,
                Err(Errno(c::EAGAIN)) => {
                    self.socket.fd.writable().await?;
                }
                Err(e) => return Err(DbusError::WriteError(e.into())),
            }
        }
        buf.clear();
        Ok(())
    }
}

fn line_to_cmd(line: &str) -> (&str, &str) {
    let line = line.trim();
    line.split_once(' ').unwrap_or((line, ""))
}
