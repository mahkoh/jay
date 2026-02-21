use {
    crate::utils::oserror::OsError,
    uapi::{OwnedFd, c, pipe2},
};

pub struct Pipe<L, R> {
    pub read: L,
    pub write: R,
}

pub fn pipe() -> Result<Pipe<OwnedFd, OwnedFd>, OsError> {
    let (read, write) = pipe2(c::O_CLOEXEC)?;
    Ok(Pipe { read, write })
}

impl<L, R> Pipe<L, R> {
    pub fn map_read<Lprime>(self, map: impl FnOnce(L) -> Lprime) -> Pipe<Lprime, R> {
        Pipe {
            read: map(self.read),
            write: self.write,
        }
    }

    pub fn map_write<Rprime>(self, map: impl FnOnce(R) -> Rprime) -> Pipe<L, Rprime> {
        Pipe {
            read: self.read,
            write: map(self.write),
        }
    }
}
