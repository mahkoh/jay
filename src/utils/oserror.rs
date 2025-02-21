use {
    once_cell::sync::Lazy,
    std::{
        error::Error,
        fmt::{Display, Formatter},
    },
    uapi::{
        Errno,
        c::{self, c_int},
    },
};

static ERRORS: Lazy<&'static [Option<&'static str>]> = Lazy::new(|| {
    static MSGS: &[(c::c_int, &str)] = &[
        (c::EWOULDBLOCK, "Operation would block"),
        (c::ENOTSUP, "Not supported"),
        (c::EHWPOISON, "Memory page has hardware error"),
        (c::ERFKILL, "Operation not possible due to RF-kill"),
        (c::EKEYREJECTED, "Key was rejected by service"),
        (c::EKEYREVOKED, "Key has been revoked"),
        (c::EKEYEXPIRED, "Key has expired"),
        (c::ENOKEY, "Required key not available"),
        (c::EMEDIUMTYPE, "Wrong medium type"),
        (c::ENOMEDIUM, "No medium found"),
        (c::EREMOTEIO, "Remote I/O error"),
        (c::EISNAM, "Is a named type file"),
        (c::ENAVAIL, "No XENIX semaphores available"),
        (c::ENOTNAM, "Not a XENIX named type file"),
        (c::EUCLEAN, "Structure needs cleaning"),
        (c::ESTRPIPE, "Streams pipe error"),
        (c::ELIBEXEC, "Cannot exec a shared library directly"),
        (
            c::ELIBMAX,
            "Attempting to link in too many shared libraries",
        ),
        (c::ELIBSCN, ".lib section in a.out corrupted"),
        (c::ELIBBAD, "Accessing a corrupted shared library"),
        (c::ELIBACC, "Can not access a needed shared library"),
        (c::EREMCHG, "Remote address changed"),
        (c::EBADFD, "File descriptor in bad state"),
        (c::ENOTUNIQ, "Name not unique on network"),
        (c::EDOTDOT, "RFS specific error"),
        (c::ECOMM, "Communication error on send"),
        (c::ESRMNT, "Srmount error"),
        (c::EADV, "Advertise error"),
        (c::ENOPKG, "Package not installed"),
        (c::ENONET, "Machine is not on the network"),
        (c::EBFONT, "Bad font file format"),
        (c::EBADSLT, "Invalid slot"),
        (c::EBADRQC, "Invalid request code"),
        (c::ENOANO, "No anode"),
        (c::EXFULL, "Exchange full"),
        (c::EBADR, "Invalid request descriptor"),
        (c::EBADE, "Invalid exchange"),
        (c::EL2HLT, "Level 2 halted"),
        (c::ENOCSI, "No CSI structure available"),
        (c::EUNATCH, "Protocol driver not attached"),
        (c::ELNRNG, "Link number out of range"),
        (c::EL3RST, "Level 3 reset"),
        (c::EL3HLT, "Level 3 halted"),
        (c::EL2NSYNC, "Level 2 not synchronized"),
        (c::ECHRNG, "Channel number out of range"),
        (c::ERESTART, "Interrupted system call should be restarted"),
        (c::ENOTRECOVERABLE, "State not recoverable"),
        (c::EOWNERDEAD, "Owner died"),
        (c::ECANCELED, "Operation canceled"),
        (c::ETIME, "Timer expired"),
        (c::EPROTO, "Protocol error"),
        (c::EOVERFLOW, "Value too large for defined data type"),
        (c::ENOSTR, "Device not a stream"),
        (c::ENOSR, "Out of streams resources"),
        (c::ENOMSG, "No message of desired type"),
        (c::ENOLINK, "Link has been severed"),
        (c::ENODATA, "No data available"),
        (c::EMULTIHOP, "Multihop attempted"),
        (c::EIDRM, "Identifier removed"),
        (c::EBADMSG, "Bad message"),
        (
            c::EILSEQ,
            "Invalid or incomplete multibyte or wide character",
        ),
        (c::ENOSYS, "Function not implemented"),
        (c::ENOLCK, "No locks available"),
        (c::EREMOTE, "Object is remote"),
        (c::ESTALE, "Stale file handle"),
        (c::EDQUOT, "Disk quota exceeded"),
        (c::EUSERS, "Too many users"),
        (c::ENOTEMPTY, "Directory not empty"),
        (c::EHOSTUNREACH, "No route to host"),
        (c::EHOSTDOWN, "Host is down"),
        (c::ENAMETOOLONG, "File name too long"),
        (c::ELOOP, "Too many levels of symbolic links"),
        (c::ECONNREFUSED, "Connection refused"),
        (c::ETIMEDOUT, "Connection timed out"),
        (c::ETOOMANYREFS, "Too many references: cannot splice"),
        (
            c::ESHUTDOWN,
            "Cannot send after transport endpoint shutdown",
        ),
        (c::EDESTADDRREQ, "Destination address required"),
        (c::ENOTCONN, "Transport endpoint is not connected"),
        (c::EISCONN, "Transport endpoint is already connected"),
        (c::ENOBUFS, "No buffer space available"),
        (c::ECONNRESET, "Connection reset by peer"),
        (c::ECONNABORTED, "Software caused connection abort"),
        (c::ENETRESET, "Network dropped connection on reset"),
        (c::ENETUNREACH, "Network is unreachable"),
        (c::ENETDOWN, "Network is down"),
        (c::EADDRNOTAVAIL, "Cannot assign requested address"),
        (c::EADDRINUSE, "Address already in use"),
        (c::EAFNOSUPPORT, "Address family not supported by protocol"),
        (c::EPFNOSUPPORT, "Protocol family not supported"),
        (c::EOPNOTSUPP, "Operation not supported"),
        (c::ESOCKTNOSUPPORT, "Socket type not supported"),
        (c::EPROTONOSUPPORT, "Protocol not supported"),
        (c::ENOPROTOOPT, "Protocol not available"),
        (c::EPROTOTYPE, "Protocol wrong type for socket"),
        (c::EMSGSIZE, "Message too long"),
        (c::ENOTSOCK, "Socket operation on non-socket"),
        (c::EALREADY, "Operation already in progress"),
        (c::EINPROGRESS, "Operation now in progress"),
        (c::EAGAIN, "Resource temporarily unavailable"),
        (c::ERANGE, "Numerical result out of range"),
        (c::EDOM, "Numerical argument out of domain"),
        (c::EPIPE, "Broken pipe"),
        (c::EMLINK, "Too many links"),
        (c::EROFS, "Read-only file system"),
        (c::ESPIPE, "Illegal seek"),
        (c::ENOSPC, "No space left on device"),
        (c::EFBIG, "File too large"),
        (c::ETXTBSY, "Text file busy"),
        (c::ENOTTY, "Inappropriate ioctl for device"),
        (c::ENFILE, "Too many open files in system"),
        (c::EMFILE, "Too many open files"),
        (c::EINVAL, "Invalid argument"),
        (c::EISDIR, "Is a directory"),
        (c::ENOTDIR, "Not a directory"),
        (c::ENODEV, "No such device"),
        (c::EXDEV, "Invalid cross-device link"),
        (c::EEXIST, "File exists"),
        (c::EBUSY, "Device or resource busy"),
        (c::ENOTBLK, "Block device required"),
        (c::EFAULT, "Bad address"),
        (c::EACCES, "Permission denied"),
        (c::ENOMEM, "Cannot allocate memory"),
        (c::EDEADLK, "Resource deadlock avoided"),
        (c::ECHILD, "No child processes"),
        (c::EBADF, "Bad file descriptor"),
        (c::ENOEXEC, "Exec format error"),
        (c::E2BIG, "Argument list too long"),
        (c::ENXIO, "No such device or address"),
        (c::EIO, "Input/output error"),
        (c::EINTR, "Interrupted system call"),
        (c::ESRCH, "No such process"),
        (c::ENOENT, "No such file or directory"),
        (c::EPERM, "Operation not permitted"),
    ];
    let mut res = vec![];
    for &(idx, msg) in MSGS {
        let idx = idx as usize;
        while res.len() <= idx {
            res.push(None);
        }
        res[idx] = Some(msg);
    }
    res.leak()
});

#[derive(Debug, Eq, PartialEq)]
pub struct OsError(pub c::c_int);

impl From<Errno> for OsError {
    fn from(e: Errno) -> Self {
        Self(e.0)
    }
}

impl From<c::c_int> for OsError {
    fn from(v: c_int) -> Self {
        Self(v)
    }
}

impl From<std::io::Error> for OsError {
    fn from(v: std::io::Error) -> Self {
        match v.raw_os_error() {
            Some(v) => Self(v),
            None => Self(c::EINVAL),
        }
    }
}

impl Default for OsError {
    fn default() -> Self {
        Errno::default().into()
    }
}

impl Error for OsError {}

impl Display for OsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = ERRORS
            .get(self.0 as usize)
            .and_then(|v| *v)
            .unwrap_or("unknown error");
        write!(f, "{} (os error {})", msg, self.0)
    }
}

#[cfg_attr(not(feature = "it"), expect(dead_code))]
pub trait OsErrorExt {
    type Container;

    fn to_os_error(self) -> Self::Container;
}

impl<T> OsErrorExt for Result<T, Errno> {
    type Container = Result<T, OsError>;

    fn to_os_error(self) -> Self::Container {
        self.map_err(|e| e.into())
    }
}
