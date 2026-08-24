#![allow(non_camel_case_types, dead_code)]

use jay_algorithms::oserror::OsError;
use std::mem::MaybeUninit;
use uapi::OwnedFd;
use uapi::c;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct io_uring_sqe {
    pub opcode: u8,
    pub flags: u8,
    pub ioprio: u16,
    pub fd: i32,
    pub u1: io_uring_sqe_union1,
    pub u2: io_uring_sqe_union2,
    pub len: u32,
    pub u3: io_uring_sqe_union3,
    pub user_data: u64,
    pub u4: io_uring_sqe_union4,
    pub personality: u16,
    pub u5: io_uring_sqe_union5,
    pub u6: io_uring_sqe_union6,
}

impl Default for io_uring_sqe {
    fn default() -> Self {
        unsafe { MaybeUninit::zeroed().assume_init() }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union io_uring_sqe_union1 {
    pub off: u64,
    pub addr2: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union io_uring_sqe_union2 {
    pub addr: u64,
    pub splice_off_in: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union io_uring_sqe_union3 {
    pub rw_flags: c::c_int,
    pub fsync_flags: u32,
    pub poll_events: u16,
    pub poll32_events: u32,
    pub sync_range_flags: u32,
    pub msg_flags: u32,
    pub timeout_flags: u32,
    pub accept_flags: u32,
    pub cancel_flags: u32,
    pub open_flags: u32,
    pub statx_flags: u32,
    pub fadvise_advice: u32,
    pub splice_flags: u32,
    pub rename_flags: u32,
    pub unlink_flags: u32,
    pub hardlink_flags: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub union io_uring_sqe_union4 {
    pub buf_index: u16,
    pub buf_group: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union io_uring_sqe_union5 {
    pub splice_fd_in: i32,
    pub file_index: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union io_uring_sqe_union6 {
    pub s1: io_uring_sqe_union6_struct1,
    pub s2: io_uring_sqe_union6_struct2,
    pub optval: u64,
    pub cmd: [u8; 0],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct io_uring_sqe_union6_struct1 {
    pub addr3: u64,
    pub __pad2: [u64; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct io_uring_sqe_union6_struct2 {
    pub attr_ptr: u64,
    pub attr_type_mask: u64,
}

pub const IOSQE_FIXED_FILE_BIT: u8 = 0;
pub const IOSQE_IO_DRAIN_BIT: u8 = 1;
pub const IOSQE_IO_LINK_BIT: u8 = 2;
pub const IOSQE_IO_HARDLINK_BIT: u8 = 3;
pub const IOSQE_ASYNC_BIT: u8 = 4;
pub const IOSQE_BUFFER_SELECT_BIT: u8 = 5;
pub const IOSQE_CQE_SKIP_SUCCESS_BIT: u8 = 6;

pub const IOSQE_FIXED_FILE: u8 = 1 << IOSQE_FIXED_FILE_BIT;
pub const IOSQE_IO_DRAIN: u8 = 1 << IOSQE_IO_DRAIN_BIT;
pub const IOSQE_IO_LINK: u8 = 1 << IOSQE_IO_LINK_BIT;
pub const IOSQE_IO_HARDLINK: u8 = 1 << IOSQE_IO_HARDLINK_BIT;
pub const IOSQE_ASYNC: u8 = 1 << IOSQE_ASYNC_BIT;
pub const IOSQE_BUFFER_SELECT: u8 = 1 << IOSQE_BUFFER_SELECT_BIT;
pub const IOSQE_CQE_SKIP_SUCCESS: u8 = 1 << IOSQE_CQE_SKIP_SUCCESS_BIT;

pub const IORING_SETUP_IOPOLL: u32 = 1 << 0;
pub const IORING_SETUP_SQPOLL: u32 = 1 << 1;
pub const IORING_SETUP_SQ_AFF: u32 = 1 << 2;
pub const IORING_SETUP_CQSIZE: u32 = 1 << 3;
pub const IORING_SETUP_CLAMP: u32 = 1 << 4;
pub const IORING_SETUP_ATTACH_WQ: u32 = 1 << 5;
pub const IORING_SETUP_R_DISABLED: u32 = 1 << 6;
pub const IORING_SETUP_SUBMIT_ALL: u32 = 1 << 7;
pub const IORING_SETUP_COOP_TASKRUN: u32 = 1 << 8;
pub const IORING_SETUP_TASKRUN_FLAG: u32 = 1 << 9;
pub const IORING_SETUP_SQE128: u32 = 1 << 10;
pub const IORING_SETUP_CQE32: u32 = 1 << 11;
pub const IORING_SETUP_SINGLE_ISSUER: u32 = 1 << 12;
pub const IORING_SETUP_DEFER_TASKRUN: u32 = 1 << 13;
pub const IORING_SETUP_NO_MMAP: u32 = 1 << 14;
pub const IORING_SETUP_REGISTERED_FD_ONLY: u32 = 1 << 15;
pub const IORING_SETUP_NO_SQARRAY: u32 = 1 << 16;
pub const IORING_SETUP_HYBRID_IOPOLL: u32 = 1 << 17;

pub const IORING_OP_NOP: u8 = 0;
pub const IORING_OP_READV: u8 = 1;
pub const IORING_OP_WRITEV: u8 = 2;
pub const IORING_OP_FSYNC: u8 = 3;
pub const IORING_OP_READ_FIXED: u8 = 4;
pub const IORING_OP_WRITE_FIXED: u8 = 5;
pub const IORING_OP_POLL_ADD: u8 = 6;
pub const IORING_OP_POLL_REMOVE: u8 = 7;
pub const IORING_OP_SYNC_FILE_RANGE: u8 = 8;
pub const IORING_OP_SENDMSG: u8 = 9;
pub const IORING_OP_RECVMSG: u8 = 10;
pub const IORING_OP_TIMEOUT: u8 = 11;
pub const IORING_OP_TIMEOUT_REMOVE: u8 = 12;
pub const IORING_OP_ACCEPT: u8 = 13;
pub const IORING_OP_ASYNC_CANCEL: u8 = 14;
pub const IORING_OP_LINK_TIMEOUT: u8 = 15;
pub const IORING_OP_CONNECT: u8 = 16;
pub const IORING_OP_FALLOCATE: u8 = 17;
pub const IORING_OP_OPENAT: u8 = 18;
pub const IORING_OP_CLOSE: u8 = 19;
pub const IORING_OP_FILES_UPDATE: u8 = 20;
pub const IORING_OP_STATX: u8 = 21;
pub const IORING_OP_READ: u8 = 22;
pub const IORING_OP_WRITE: u8 = 23;
pub const IORING_OP_FADVISE: u8 = 24;
pub const IORING_OP_MADVISE: u8 = 25;
pub const IORING_OP_SEND: u8 = 26;
pub const IORING_OP_RECV: u8 = 27;
pub const IORING_OP_OPENAT2: u8 = 28;
pub const IORING_OP_EPOLL_CTL: u8 = 29;
pub const IORING_OP_SPLICE: u8 = 30;
pub const IORING_OP_PROVIDE_BUFFERS: u8 = 31;
pub const IORING_OP_REMOVE_BUFFERS: u8 = 32;
pub const IORING_OP_TEE: u8 = 33;
pub const IORING_OP_SHUTDOWN: u8 = 34;
pub const IORING_OP_RENAMEAT: u8 = 35;
pub const IORING_OP_UNLINKAT: u8 = 36;
pub const IORING_OP_MKDIRAT: u8 = 37;
pub const IORING_OP_SYMLINKAT: u8 = 38;
pub const IORING_OP_LINKAT: u8 = 39;
pub const IORING_OP_MSG_RING: u8 = 40;
pub const IORING_OP_FSETXATTR: u8 = 41;
pub const IORING_OP_SETXATTR: u8 = 42;
pub const IORING_OP_FGETXATTR: u8 = 43;
pub const IORING_OP_GETXATTR: u8 = 44;
pub const IORING_OP_SOCKET: u8 = 45;
pub const IORING_OP_URING_CMD: u8 = 46;
pub const IORING_OP_SEND_ZC: u8 = 47;
pub const IORING_OP_SENDMSG_ZC: u8 = 48;
pub const IORING_OP_READ_MULTISHOT: u8 = 49;
pub const IORING_OP_WAITID: u8 = 50;
pub const IORING_OP_FUTEX_WAIT: u8 = 51;
pub const IORING_OP_FUTEX_WAKE: u8 = 52;
pub const IORING_OP_FUTEX_WAITV: u8 = 53;
pub const IORING_OP_FIXED_FD_INSTALL: u8 = 54;
pub const IORING_OP_FTRUNCATE: u8 = 55;
pub const IORING_OP_BIND: u8 = 56;
pub const IORING_OP_LISTEN: u8 = 57;
pub const IORING_OP_RECV_ZC: u8 = 58;
pub const IORING_OP_EPOLL_WAIT: u8 = 59;
pub const IORING_OP_READV_FIXED: u8 = 60;
pub const IORING_OP_WRITEV_FIXED: u8 = 61;
pub const IORING_OP_PIPE: u8 = 62;
pub const IORING_OP_NOP128: u8 = 63;
pub const IORING_OP_URING_CMD128: u8 = 64;
pub const IORING_OP_LAST: u8 = 65;

pub const IORING_FSYNC_DATASYNC: u32 = 1 << 0;

pub const IORING_TIMEOUT_ABS: u32 = 1 << 0;
pub const IORING_TIMEOUT_UPDATE: u32 = 1 << 1;
pub const IORING_TIMEOUT_BOOTTIME: u32 = 1 << 2;
pub const IORING_TIMEOUT_REALTIME: u32 = 1 << 3;
pub const IORING_LINK_TIMEOUT_UPDATE: u32 = 1 << 4;
pub const IORING_TIMEOUT_ETIME_SUCCESS: u32 = 1 << 5;
pub const IORING_TIMEOUT_CLOCK_MASK: u32 = IORING_TIMEOUT_BOOTTIME | IORING_TIMEOUT_REALTIME;
pub const IORING_TIMEOUT_UPDATE_MASK: u32 = IORING_TIMEOUT_UPDATE | IORING_LINK_TIMEOUT_UPDATE;

pub const SPLICE_F_FD_IN_FIXED: u32 = 1 << 31;

pub const IORING_POLL_ADD_MULTI: u32 = 1 << 0;
pub const IORING_POLL_UPDATE_EVENTS: u32 = 1 << 1;
pub const IORING_POLL_UPDATE_USER_DATA: u32 = 1 << 2;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_cqe {
    pub user_data: u64,
    pub res: i32,
    pub flags: u32,
}

pub const IORING_CQE_F_BUFFER: u32 = 1 << 0;
pub const IORING_CQE_F_MORE: u32 = 1 << 1;

pub const IORING_CQE_BUFFER_SHIFT: u32 = 16;

pub const IORING_OFF_SQ_RING: u64 = 0;
pub const IORING_OFF_CQ_RING: u64 = 0x8000000;
pub const IORING_OFF_SQES: u64 = 0x10000000;
pub const IORING_OFF_PBUF_RING: u64 = 0x80000000;
pub const IORING_OFF_PBUF_SHIFT: u64 = 16;
pub const IORING_OFF_MMAP_MASK: u64 = 0xf8000000;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct io_sqring_offsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub flags: u32,
    pub dropped: u32,
    pub array: u32,
    pub resv1: u32,
    pub resv2: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct io_cqring_offsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub overflow: u32,
    pub cqes: u32,
    pub flags: u32,
    pub resv1: u32,
    pub resv2: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct io_uring_params {
    pub sq_entries: u32,
    pub cq_entries: u32,
    pub flags: u32,
    pub sq_thread_cpu: u32,
    pub sq_thread_idle: u32,
    pub features: u32,
    pub wq_fd: u32,
    pub resv: [u32; 3],
    pub sq_off: io_sqring_offsets,
    pub cq_off: io_cqring_offsets,
}

pub const IORING_SQ_NEED_WAKEUP: u32 = 1 << 0;
pub const IORING_SQ_CQ_OVERFLOW: u32 = 1 << 1;

pub const IORING_CQ_EVENTFD_DISABLED: u32 = 1 << 0;

pub const IORING_ENTER_GETEVENTS: c::c_uint = 1 << 0;
pub const IORING_ENTER_SQ_WAKEUP: c::c_uint = 1 << 1;
pub const IORING_ENTER_SQ_WAIT: c::c_uint = 1 << 2;
pub const IORING_ENTER_EXT_ARG: c::c_uint = 1 << 3;

pub const IORING_FEAT_SINGLE_MMAP: u32 = 1 << 0;
pub const IORING_FEAT_NODROP: u32 = 1 << 1;
pub const IORING_FEAT_SUBMIT_STABLE: u32 = 1 << 2;
pub const IORING_FEAT_RW_CUR_POS: u32 = 1 << 3;
pub const IORING_FEAT_CUR_PERSONALITY: u32 = 1 << 4;
pub const IORING_FEAT_FAST_POLL: u32 = 1 << 5;
pub const IORING_FEAT_POLL_32BITS: u32 = 1 << 6;
pub const IORING_FEAT_SQPOLL_NONFIXED: u32 = 1 << 7;
pub const IORING_FEAT_EXT_ARG: u32 = 1 << 8;
pub const IORING_FEAT_NATIVE_WORKERS: u32 = 1 << 9;
pub const IORING_FEAT_RSRC_TAGS: u32 = 1 << 10;
pub const IORING_FEAT_CQE_SKIP: u32 = 1 << 11;

pub const IORING_REGISTER_BUFFERS: c::c_uint = 0;
pub const IORING_UNREGISTER_BUFFERS: c::c_uint = 1;
pub const IORING_REGISTER_FILES: c::c_uint = 2;
pub const IORING_UNREGISTER_FILES: c::c_uint = 3;
pub const IORING_REGISTER_EVENTFD: c::c_uint = 4;
pub const IORING_UNREGISTER_EVENTFD: c::c_uint = 5;
pub const IORING_REGISTER_FILES_UPDATE: c::c_uint = 6;
pub const IORING_REGISTER_EVENTFD_ASYNC: c::c_uint = 7;
pub const IORING_REGISTER_PROBE: c::c_uint = 8;
pub const IORING_REGISTER_PERSONALITY: c::c_uint = 9;
pub const IORING_UNREGISTER_PERSONALITY: c::c_uint = 10;
pub const IORING_REGISTER_RESTRICTIONS: c::c_uint = 11;
pub const IORING_REGISTER_ENABLE_RINGS: c::c_uint = 12;
pub const IORING_REGISTER_FILES2: c::c_uint = 13;
pub const IORING_REGISTER_FILES_UPDATE2: c::c_uint = 14;
pub const IORING_REGISTER_BUFFERS2: c::c_uint = 15;
pub const IORING_REGISTER_BUFFERS_UPDATE: c::c_uint = 16;
pub const IORING_REGISTER_IOWQ_AFF: c::c_uint = 17;
pub const IORING_UNREGISTER_IOWQ_AFF: c::c_uint = 18;
pub const IORING_REGISTER_IOWQ_MAX_WORKERS: c::c_uint = 19;
pub const IORING_REGISTER_RING_FDS: c::c_uint = 20;
pub const IORING_UNREGISTER_RING_FDS: c::c_uint = 21;
pub const IORING_REGISTER_PBUF_RING: c::c_uint = 22;
pub const IORING_UNREGISTER_PBUF_RING: c::c_uint = 23;
pub const IORING_REGISTER_SYNC_CANCEL: c::c_uint = 24;
pub const IORING_REGISTER_FILE_ALLOC_RANGE: c::c_uint = 25;
pub const IORING_REGISTER_PBUF_STATUS: c::c_uint = 26;
pub const IORING_REGISTER_NAPI: c::c_uint = 27;
pub const IORING_UNREGISTER_NAPI: c::c_uint = 28;
pub const IORING_REGISTER_CLOCK: c::c_uint = 29;
pub const IORING_REGISTER_CLONE_BUFFERS: c::c_uint = 30;
pub const IORING_REGISTER_SEND_MSG_RING: c::c_uint = 31;
pub const IORING_REGISTER_ZCRX_IFQ: c::c_uint = 32;
pub const IORING_REGISTER_RESIZE_RINGS: c::c_uint = 33;
pub const IORING_REGISTER_MEM_REGION: c::c_uint = 34;
pub const IORING_REGISTER_QUERY: c::c_uint = 35;
pub const IORING_REGISTER_ZCRX_CTRL: c::c_uint = 36;
pub const IORING_REGISTER_BPF_FILTER: c::c_uint = 37;
pub const IORING_REGISTER_USE_REGISTERED_RING: c::c_uint = 1 << 31;

pub const IO_WQ_BOUND: u32 = 0;
pub const IO_WQ_UNBOUND: u32 = 1;

#[repr(C, align(8))]
#[derive(Debug, Copy, Clone)]
pub struct AlignedU64(pub u64);

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_files_update {
    pub offset: u32,
    pub resv: u32,
    pub fds: AlignedU64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_rsrc_register {
    pub nr: u32,
    pub resv: u32,
    pub resv2: u64,
    pub data: AlignedU64,
    pub tags: AlignedU64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_rsrc_update {
    pub offset: u32,
    pub resv: u32,
    pub data: AlignedU64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_rsrc_update2 {
    pub offset: u32,
    pub resv: u32,
    pub data: AlignedU64,
    pub tags: AlignedU64,
    pub nr: u32,
    pub resv2: u32,
}

pub const IORING_REGISTER_FILES_SKIP: i32 = -2;

pub const IO_URING_OP_SUPPORTED: u32 = 1 << 0;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_probe_op {
    pub op: u8,
    pub resv: u8,
    pub flags: u16,
    pub resv2: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct io_uring_probe {
    pub last_op: u8,
    pub ops_len: u8,
    pub resv: u16,
    pub resv2: [u32; 3],
    pub ops: [io_uring_probe_op; 0],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct io_uring_restriction {
    pub opcode: u16,
    pub u1: io_uring_restriction_union1,
    pub resv: u8,
    pub resv2: [u32; 3],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union io_uring_restriction_union1 {
    pub register_op: u8,
    pub sqe_op: u8,
    pub sqe_flags: u8,
}

pub const IORING_RESTRICTION_REGISTER_OP: u16 = 0;
pub const IORING_RESTRICTION_SQE_OP: u16 = 1;
pub const IORING_RESTRICTION_SQE_FLAGS_ALLOWED: u16 = 2;
pub const IORING_RESTRICTION_SQE_FLAGS_REQUIRED: u16 = 3;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_getevents_arg {
    sigmask: u64,
    sigmask_sz: u32,
    pad: u32,
    ts: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_buf {
    pub addr: u64,
    pub len: u32,
    pub bid: u16,
    pub resv: u16,
}

pub const IOU_PBUF_RING_MMAP: u16 = 1;
pub const IOU_PBUF_RING_INC: u16 = 2;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct io_uring_buf_reg {
    pub ring_addr: u64,
    pub ring_entries: u32,
    pub bgid: u16,
    pub flags: u16,
    pub min_left: u32,
    pub resv: [u32; 5],
}

pub fn io_uring_setup(entries: u32, params: &mut io_uring_params) -> Result<OwnedFd, OsError> {
    let res = unsafe {
        c::syscall(
            c::SYS_io_uring_setup,
            entries as usize,
            params as *mut _ as usize,
        )
    };
    if res < 0 {
        Err(OsError::default())
    } else {
        Ok(OwnedFd::new(res as _))
    }
}

pub fn io_uring_enter(
    fd: c::c_int,
    to_submit: c::c_uint,
    min_complete: c::c_uint,
    flags: c::c_uint,
) -> Result<usize, OsError> {
    let res = unsafe {
        c::syscall(
            c::SYS_io_uring_enter,
            fd as usize,
            to_submit as usize,
            min_complete as usize,
            flags as usize,
            0usize,
            0usize,
        )
    };
    if res < 0 {
        Err(OsError::default())
    } else {
        Ok(res as usize)
    }
}

pub unsafe fn io_uring_register(
    fd: c::c_int,
    opcode: c::c_uint,
    arg: *mut u8,
    nr_args: c::c_uint,
) -> Result<c::c_int, OsError> {
    let res = unsafe {
        c::syscall(
            c::SYS_io_uring_register,
            fd as usize,
            opcode as usize,
            arg as usize,
            nr_args as usize,
        )
    };
    if res < 0 {
        Err(OsError::default())
    } else {
        Ok(res as c::c_int)
    }
}
