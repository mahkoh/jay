use {
    crate::{
        dbus::{DbusError, DbusSocket, incoming::handle_incoming, outgoing::handle_outgoing},
        utils::{buf::Buf, errorfmt::ErrorFmt, hex},
    },
    std::{ops::Deref, rc::Rc},
};

pub(super) async fn handle_auth(socket: Rc<DbusSocket>) {
    let mut auth = Auth {
        socket: socket.clone(),
        buf: Buf::new(BUF_SIZE),
        buf_start: 0,
        buf_stop: 0,
    };
    auth.run().await;
}

const BUF_SIZE: usize = 128;

struct Auth {
    socket: Rc<DbusSocket>,

    buf: Buf,
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
            self.socket
                .eng
                .spawn("dbus incoming", handle_incoming(self.socket.clone())),
        ));
        self.socket.outgoing_.set(Some(
            self.socket
                .eng
                .spawn("dbus outgoing", handle_outgoing(self.socket.clone())),
        ));
        self.socket.auth.take();
    }

    async fn handle_auth(&mut self) -> Result<(), DbusError> {
        let uid = hex::to_hex(&uapi::getuid().to_string());
        let mut out_buf = Buf::new(128);
        {
            let buf = out_buf
                .write_fmt(format_args!("\0AUTH EXTERNAL {}\r\n", uid))
                .unwrap();
            self.write_buf(buf).await?;
        }
        let line = self.readline().await?;
        let (cmd, _) = line_to_cmd(&line);
        if cmd != "OK" {
            return Err(DbusError::Auth);
        }
        {
            let buf = out_buf
                .write_fmt(format_args!("NEGOTIATE_UNIX_FD\r\n"))
                .unwrap();
            self.write_buf(buf).await?;
        }
        let line = self.readline().await?;
        let (cmd, _) = line_to_cmd(&line);
        if cmd != "AGREE_UNIX_FD" {
            return Err(DbusError::UnixFd);
        }
        {
            let buf = out_buf.write_fmt(format_args!("BEGIN\r\n")).unwrap();
            self.write_buf(buf).await?;
        }
        Ok(())
    }

    async fn readline(&mut self) -> Result<String, DbusError> {
        let mut s = String::new();
        loop {
            {
                let buf = self.buf.deref();
                for i in self.buf_start..self.buf_stop {
                    let c = buf[i % BUF_SIZE] as char;
                    s.push(c);
                    if c == '\n' {
                        self.buf_start = i + 1;
                        return Ok(s);
                    }
                }
            }
            self.buf_start = 0;
            self.buf_stop = 0;
            let res = self
                .socket
                .ring
                .read(&self.socket.fd, self.buf.clone())
                .await;
            match res {
                Ok(n) => self.buf_stop = n,
                Err(e) => return Err(DbusError::ReadError(e)),
            }
        }
    }

    async fn write_buf(&mut self, mut buf: Buf) -> Result<(), DbusError> {
        let mut start = 0;
        while start < buf.len() {
            let res = self
                .socket
                .ring
                .write(&self.socket.fd, buf.slice(start..), None)
                .await;
            match res {
                Ok(n) => start += n,
                Err(e) => return Err(DbusError::WriteError(e)),
            }
        }
        Ok(())
    }
}

fn line_to_cmd(line: &str) -> (&str, &str) {
    let line = line.trim();
    line.split_once(' ').unwrap_or((line, ""))
}
