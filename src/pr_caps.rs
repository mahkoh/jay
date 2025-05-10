use {
    crate::{
        pr_caps::sys::{
            _LINUX_CAPABILITY_U32S_3, _LINUX_CAPABILITY_VERSION_3, CAP_SYS_NICE, cap_user_data_t,
            cap_user_header_t,
        },
        utils::{bitflags::BitflagsExt, errorfmt::ErrorFmt, oserror::OsError},
    },
    uapi::{
        c::{SYS_capget, SYS_capset, syscall},
        map_err,
    },
};

pub struct PrCaps {
    effective: u64,
    permitted: u64,
    inheritable: u64,
}

pub struct PrCompCaps {
    caps: PrCaps,
}

pub fn pr_caps() -> PrCaps {
    let mut hdr = cap_user_header_t {
        version: _LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let mut caps = [cap_user_data_t::default(); _LINUX_CAPABILITY_U32S_3];
    let ret = unsafe { syscall(SYS_capget, &mut hdr, &mut caps) };
    if let Err(e) = map_err!(ret) {
        eprintln!(
            "Could not get process capabilities: {}",
            ErrorFmt(OsError(e.0))
        );
        return PrCaps {
            effective: 0,
            permitted: 0,
            inheritable: 0,
        };
    }
    PrCaps {
        effective: caps[0].effective as u64 | ((caps[1].effective as u64) << 32),
        permitted: caps[0].permitted as u64 | ((caps[1].permitted as u64) << 32),
        inheritable: caps[0].inheritable as u64 | ((caps[1].inheritable as u64) << 32),
    }
}

pub fn drop_all_pr_caps() {
    let mut hdr = cap_user_header_t {
        version: _LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let caps = [cap_user_data_t::default(); _LINUX_CAPABILITY_U32S_3];
    let ret = unsafe { syscall(SYS_capset, &mut hdr, &caps) };
    if let Err(e) = map_err!(ret) {
        eprintln!(
            "Could not get drop capabilities: {}",
            ErrorFmt(OsError(e.0))
        );
    }
}

impl PrCaps {
    pub fn into_comp(mut self) -> PrCompCaps {
        let mut caps = 0;
        macro_rules! add_cap {
            ($name:ident) => {
                if self.permitted.contains(1 << $name) {
                    caps |= 1 << $name;
                }
            };
        }
        add_cap!(CAP_SYS_NICE);
        let mut hdr = cap_user_header_t {
            version: _LINUX_CAPABILITY_VERSION_3,
            pid: 0,
        };
        let caps_hi = (caps >> 32) as u32;
        let caps_lo = caps as u32;
        let mut data = [cap_user_data_t::default(); _LINUX_CAPABILITY_U32S_3];
        data[0].effective = caps_lo;
        data[1].effective = caps_hi;
        data[0].permitted = caps_lo;
        data[1].permitted = caps_hi;
        let ret = unsafe { syscall(SYS_capset, &mut hdr, &data) };
        if let Err(e) = map_err!(ret) {
            eprintln!(
                "Could not get set compositor capabilities: {}",
                ErrorFmt(OsError(e.0))
            );
            return PrCompCaps { caps: self };
        }
        self.effective = caps;
        self.permitted = caps;
        self.inheritable = 0;
        PrCompCaps { caps: self }
    }
}

impl PrCompCaps {
    pub fn has_nice(&self) -> bool {
        self.caps.effective.contains(1 << CAP_SYS_NICE)
    }
}

impl Drop for PrCaps {
    fn drop(&mut self) {
        drop_all_pr_caps();
    }
}

mod sys {
    #![allow(dead_code)]

    use uapi::c::pid_t;

    pub const _LINUX_CAPABILITY_VERSION_3: u32 = 0x20080522;
    pub const _LINUX_CAPABILITY_U32S_3: usize = 2;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct cap_user_header_t {
        pub version: u32,
        pub pid: pid_t,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    pub struct cap_user_data_t {
        pub effective: u32,
        pub permitted: u32,
        pub inheritable: u32,
    }

    pub const CAP_CHOWN: u32 = 0;
    pub const CAP_DAC_OVERRIDE: u32 = 1;
    pub const CAP_DAC_READ_SEARCH: u32 = 2;
    pub const CAP_FOWNER: u32 = 3;
    pub const CAP_FSETID: u32 = 4;
    pub const CAP_KILL: u32 = 5;
    pub const CAP_SETGID: u32 = 6;
    pub const CAP_SETUID: u32 = 7;
    pub const CAP_SETPCAP: u32 = 8;
    pub const CAP_LINUX_IMMUTABLE: u32 = 9;
    pub const CAP_NET_BIND_SERVICE: u32 = 10;
    pub const CAP_NET_BROADCAST: u32 = 11;
    pub const CAP_NET_ADMIN: u32 = 12;
    pub const CAP_NET_RAW: u32 = 13;
    pub const CAP_IPC_LOCK: u32 = 14;
    pub const CAP_IPC_OWNER: u32 = 15;
    pub const CAP_SYS_MODULE: u32 = 16;
    pub const CAP_SYS_RAWIO: u32 = 17;
    pub const CAP_SYS_CHROOT: u32 = 18;
    pub const CAP_SYS_PTRACE: u32 = 19;
    pub const CAP_SYS_PACCT: u32 = 20;
    pub const CAP_SYS_ADMIN: u32 = 21;
    pub const CAP_SYS_BOOT: u32 = 22;
    pub const CAP_SYS_NICE: u32 = 23;
    pub const CAP_SYS_RESOURCE: u32 = 24;
    pub const CAP_SYS_TIME: u32 = 25;
    pub const CAP_SYS_TTY_CONFIG: u32 = 26;
    pub const CAP_MKNOD: u32 = 27;
    pub const CAP_LEASE: u32 = 28;
    pub const CAP_AUDIT_WRITE: u32 = 29;
    pub const CAP_AUDIT_CONTROL: u32 = 30;
    pub const CAP_SETFCAP: u32 = 31;
    pub const CAP_MAC_OVERRIDE: u32 = 32;
    pub const CAP_MAC_ADMIN: u32 = 33;
    pub const CAP_SYSLOG: u32 = 34;
    pub const CAP_WAKE_ALARM: u32 = 35;
    pub const CAP_BLOCK_SUSPEND: u32 = 36;
    pub const CAP_AUDIT_READ: u32 = 37;
    pub const CAP_PERFMON: u32 = 38;
    pub const CAP_BPF: u32 = 39;
    pub const CAP_CHECKPOINT_RESTORE: u32 = 40;
}
