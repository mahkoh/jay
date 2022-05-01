pub use leaks::*;

macro_rules! track {
    ($client:expr, $rc:expr) => {
        $rc.tracker.register($client.id);
    };
}

#[cfg(not(feature = "rc_tracking"))]
mod leaks {
    use {crate::client::ClientId, std::marker::PhantomData};

    pub fn init() {
        // nothing
    }

    pub fn log_leaked() {
        // nothing
    }

    pub struct Tracker<T> {
        _phantom: PhantomData<T>,
    }

    impl<T> Tracker<T> {
        pub fn register(&self, _client: ClientId) {
            // nothing
        }
    }

    impl<T> Default for Tracker<T> {
        fn default() -> Self {
            Self {
                _phantom: Default::default(),
            }
        }
    }
}

#[cfg(feature = "rc_tracking")]
mod leaks {
    use {
        crate::{
            client::ClientId,
            utils::ptr_ext::{MutPtrExt, PtrExt},
        },
        ahash::{AHashMap, AHashSet},
        backtrace::Backtrace,
        std::{
            alloc::{GlobalAlloc, Layout},
            any,
            marker::PhantomData,
            mem, ptr,
        },
        uapi::c,
    };

    #[thread_local]
    static mut MAP: *mut AHashMap<u64, Tracked> = ptr::null_mut();

    #[thread_local]
    static mut ID: u64 = 0;

    pub fn init() {
        unsafe {
            MAP = Box::into_raw(Box::new(AHashMap::new()));
            ALLOCATIONS = Box::into_raw(Box::new(AHashMap::new()));
            IN_ALLOCATOR = 0;
            INITIALIZED = true;
        }
    }

    fn log_containers(
        prefix: &str,
        allocation: &mut Allocation,
        offset: usize,
        logged: &mut AHashSet<*mut u8>,
    ) {
        log::info!(
            "{}Contained in allocation {:?} at offset {}. Backtrace:",
            prefix,
            allocation.addr,
            offset
        );
        allocation.backtrace.resolve();
        let bt = format!("{:?}", allocation.backtrace);
        for line in bt.lines() {
            log::info!("{}    {}", prefix, line);
        }

        if !logged.insert(allocation.addr) {
            log::error!("{} LOOP", prefix);
        } else {
            let containers = find_allocations_pointing_to(allocation.addr);
            if containers.is_empty() {
                log::error!("{} NO REFERENCES", prefix);
            }
            let new_prefix = format!("{}    ", prefix);
            for (mut allocation, offset) in containers {
                log_containers(&new_prefix, &mut allocation, offset, logged);
            }
            logged.remove(&allocation.addr);
        }
    }

    pub fn log_leaked() {
        unsafe {
            IN_ALLOCATOR += 1;
            let mut map: AHashMap<ClientId, Vec<(u64, Tracked)>> = AHashMap::new();
            for (id, obj) in MAP.deref_mut().drain() {
                map.entry(obj.client).or_default().push((id, obj));
            }
            if map.is_empty() {
                log::info!("No leaks");
            }
            for (_, mut objs) in map.drain() {
                if objs.len() == 0 {
                    continue;
                }
                objs.sort_by_key(|o| o.0);
                log::info!("Client {} leaked {} objects", objs[0].1.client, objs.len());
                for (_, obj) in objs {
                    let time = chrono::NaiveDateTime::from_timestamp(obj.time.0, obj.time.1);
                    log::info!("  [{}] {}", time.format("%H:%M:%S%.3f"), obj.ty,);
                    match find_allocation_containing(obj.addr) {
                        Some(mut alloc) => {
                            log_containers("    ", &mut alloc, 0, &mut AHashSet::new())
                        }
                        _ => log::error!("    Not contained in any allocation??"),
                    }
                }
            }
            IN_ALLOCATOR -= 1;
        }
    }

    #[derive(Copy, Clone)]
    struct Tracked {
        addr: usize,
        client: ClientId,
        ty: &'static str,
        time: (i64, u32),
    }

    pub struct Tracker<T> {
        id: u64,
        _phantom: PhantomData<T>,
    }

    impl<T> Default for Tracker<T> {
        fn default() -> Self {
            Self {
                id: unsafe {
                    let id = ID;
                    ID += 1;
                    id
                },
                _phantom: Default::default(),
            }
        }
    }

    impl<T> Tracker<T> {
        pub fn register(&self, client_id: ClientId) {
            unsafe {
                let mut time = c::timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                };
                uapi::clock_gettime(c::CLOCK_REALTIME, &mut time).unwrap();
                IN_ALLOCATOR += 1;
                MAP.deref_mut().insert(
                    self.id,
                    Tracked {
                        addr: self as *const _ as usize,
                        client: client_id,
                        ty: any::type_name::<T>(),
                        time: (time.tv_sec as i64, time.tv_nsec as u32),
                    },
                );
                IN_ALLOCATOR -= 1;
            }
        }
    }

    impl<T> Drop for Tracker<T> {
        fn drop(&mut self) {
            unsafe {
                MAP.deref_mut().remove(&self.id);
            }
        }
    }

    struct TracingAllocator;

    #[global_allocator]
    static GLOBAL: TracingAllocator = TracingAllocator;

    #[derive(Clone)]
    struct Allocation {
        pub addr: *mut u8,
        pub len: usize,
        pub backtrace: Backtrace,
    }

    #[thread_local]
    static mut ALLOCATIONS: *mut AHashMap<*mut u8, Allocation> = ptr::null_mut();

    #[thread_local]
    static mut IN_ALLOCATOR: u32 = 1;

    #[thread_local]
    static mut INITIALIZED: bool = false;

    unsafe impl GlobalAlloc for TracingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let res = c::calloc(layout.size(), 1) as *mut u8;
            if IN_ALLOCATOR == 0 {
                IN_ALLOCATOR = 1;
                ALLOCATIONS.deref_mut().insert(
                    res,
                    Allocation {
                        addr: res,
                        len: layout.size(),
                        backtrace: Backtrace::new_unresolved(),
                    },
                );
                IN_ALLOCATOR = 0;
            }
            res
        }

        unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
            if INITIALIZED {
                ALLOCATIONS.deref_mut().remove(&ptr);
            }
            // c::memset(ptr as _, 0, layout.size());
            c::free(ptr as _);
        }
    }

    fn find_allocations_pointing_to(addr: *mut u8) -> Vec<(Allocation, usize)> {
        unsafe {
            IN_ALLOCATOR += 1;
            let mut res = vec![];
            for allocation in ALLOCATIONS.deref().values() {
                let num = allocation.len / mem::size_of::<usize>();
                let elements = std::slice::from_raw_parts(allocation.addr as *const *mut u8, num);
                for (offset, pos) in elements.iter().enumerate() {
                    if *pos == addr {
                        res.push((allocation.clone(), offset * mem::size_of::<usize>()));
                        break;
                    }
                }
            }
            IN_ALLOCATOR -= 1;
            res
        }
    }

    fn find_allocation_containing(addr: usize) -> Option<Allocation> {
        unsafe {
            IN_ALLOCATOR += 1;
            let mut res = None;
            for allocation in ALLOCATIONS.deref().values() {
                let aaddr = allocation.addr as usize;
                if aaddr <= addr && addr < aaddr + allocation.len {
                    res = Some(allocation.clone());
                    break;
                }
            }
            IN_ALLOCATOR -= 1;
            res
        }
    }
}
