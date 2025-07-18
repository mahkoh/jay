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
            utils::{
                hash_map_ext::HashMapExt,
                ptr_ext::{MutPtrExt, PtrExt},
                windows::WindowsExt,
            },
        },
        ahash::{AHashMap, AHashSet},
        backtrace::Backtrace,
        std::{
            alloc::{GlobalAlloc, Layout},
            any,
            cell::Cell,
            marker::PhantomData,
            ptr,
        },
        uapi::c,
    };

    thread_local! {
        static MAP: Cell<*mut AHashMap<u64, Tracked>> = const { Cell::new(ptr::null_mut()) };
        static ID: Cell<u64> = const { Cell::new(0) };
    }

    pub fn init() {
        if INITIALIZED.get() {
            return;
        }
        MAP.set(Box::into_raw(Box::new(AHashMap::new())));
        ALLOCATIONS.set(Box::into_raw(Box::new(AHashMap::new())));
        IN_ALLOCATOR.set(0);
        INITIALIZED.set(true);
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
                let mut frames = vec![];
                backtrace::trace(|frame| {
                    frames.push((frame.ip() as usize, frame.sp() as usize));
                    true
                });
                let mut frames2 = vec![];
                for [l, r] in frames.array_windows_ext::<2>() {
                    frames2.push((l.0, l.1, r.1));
                }
                let mut referenced_on_stack = false;
                for (ip, lo, hi) in frames2 {
                    if lo % 8 != 0 {
                        log::error!("lo % 8 != 0");
                    }
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            lo as *const *mut u8,
                            (hi - lo) / size_of::<usize>(),
                        )
                    };
                    for addr in slice {
                        if *addr == allocation.addr {
                            let mut name = String::new();
                            backtrace::resolve(ip as _, |sym| {
                                let symname = match sym.name() {
                                    Some(s) => s.to_string(),
                                    _ => String::new(),
                                };
                                name =
                                    format!("{} {:?}:{:?}", symname, sym.filename(), sym.lineno())
                            });
                            if !name.starts_with("jay::leaks::") {
                                log::info!("{} REFERENCED ON THE STACK: {}", prefix, name);
                                referenced_on_stack = true;
                            }
                        }
                    }
                }
                if !referenced_on_stack {
                    log::error!("{} NO REFERENCES", prefix);
                }
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
            IN_ALLOCATOR.set(IN_ALLOCATOR.get() + 1);
            let mut map: AHashMap<ClientId, Vec<(u64, Tracked)>> = AHashMap::new();
            for (id, obj) in MAP.get().deref_mut().drain() {
                map.entry(obj.client).or_default().push((id, obj));
            }
            if map.is_empty() {
                log::info!("No leaks");
            }
            for mut objs in map.drain_values() {
                if objs.len() == 0 {
                    continue;
                }
                objs.sort_by_key(|o| o.0);
                log::info!("Client {} leaked {} objects", objs[0].1.client, objs.len());
                for (_, obj) in objs {
                    let time = chrono::DateTime::from_timestamp(obj.time.0, obj.time.1)
                        .unwrap()
                        .naive_utc();
                    log::info!("  [{}] {}", time.format("%H:%M:%S%.3f"), obj.ty,);
                    match find_allocation_containing(obj.addr) {
                        Some(mut alloc) => {
                            log_containers("    ", &mut alloc, 0, &mut AHashSet::new())
                        }
                        _ => log::error!("    Not contained in any allocation??"),
                    }
                }
            }
            IN_ALLOCATOR.set(IN_ALLOCATOR.get() - 1);
        }
    }
    //
    // pub fn log_allocations(w: &mut dyn Write) {
    //     log::info!("remaining allocations:");
    //     unsafe {
    //         IN_ALLOCATOR += 1;
    //         for (_, a) in ALLOCATIONS.deref() {
    //             let mut bt = a.backtrace.clone();
    //             bt.resolve();
    //             write!(w, "[{:?}, {:?}), allocated at\n{:?}", a.addr, a.addr.add(a.len), bt);
    //         }
    //         IN_ALLOCATOR -= 1;
    //     }
    // }

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
                id: {
                    let id = ID.get();
                    ID.set(id + 1);
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
                IN_ALLOCATOR.set(IN_ALLOCATOR.get() + 1);
                MAP.get().deref_mut().insert(
                    self.id,
                    Tracked {
                        addr: self as *const _ as usize,
                        client: client_id,
                        ty: any::type_name::<T>(),
                        time: (time.tv_sec as i64, time.tv_nsec as u32),
                    },
                );
                IN_ALLOCATOR.set(IN_ALLOCATOR.get() - 1);
            }
        }
    }

    impl<T> Drop for Tracker<T> {
        fn drop(&mut self) {
            unsafe {
                MAP.get().deref_mut().remove(&self.id);
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

    thread_local! {
        static ALLOCATIONS: Cell<*mut AHashMap<*mut u8, Allocation>> = const { Cell::new(ptr::null_mut()) };
        static IN_ALLOCATOR: Cell<u32> = const { Cell::new(1) };
        static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    }

    unsafe impl GlobalAlloc for TracingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe {
                let res = c::aligned_alloc(layout.align(), layout.size()) as *mut u8;
                c::memset(res.cast(), 0, layout.size());
                if IN_ALLOCATOR.get() == 0 {
                    IN_ALLOCATOR.set(1);
                    ALLOCATIONS.get().deref_mut().insert(
                        res,
                        Allocation {
                            addr: res,
                            len: layout.size(),
                            backtrace: Backtrace::new_unresolved(),
                        },
                    );
                    // log::info!("allocated [0x{:x}, 0x{:x})", res as usize, res as usize + layout.size());
                    IN_ALLOCATOR.set(0);
                }
                res
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
            unsafe {
                if INITIALIZED.get() {
                    ALLOCATIONS.get().deref_mut().remove(&ptr);
                }
                // c::memset(ptr as _, 0, layout.size());
                c::free(ptr as _);
            }
        }
    }

    fn find_allocations_pointing_to(addr: *mut u8) -> Vec<(Allocation, usize)> {
        unsafe {
            IN_ALLOCATOR.set(IN_ALLOCATOR.get() + 1);
            let mut res = vec![];
            for allocation in ALLOCATIONS.get().deref().values() {
                let num = allocation.len / size_of::<usize>();
                let elements = std::slice::from_raw_parts(allocation.addr as *const *mut u8, num);
                for (offset, pos) in elements.iter().enumerate() {
                    if *pos == addr {
                        res.push((allocation.clone(), offset * size_of::<usize>()));
                        break;
                    }
                }
            }
            IN_ALLOCATOR.set(IN_ALLOCATOR.get() - 1);
            res
        }
    }

    fn find_allocation_containing(addr: usize) -> Option<Allocation> {
        unsafe {
            IN_ALLOCATOR.set(IN_ALLOCATOR.get() + 1);
            let mut res = None;
            for allocation in ALLOCATIONS.get().deref().values() {
                let aaddr = allocation.addr as usize;
                if aaddr <= addr && addr < aaddr + allocation.len {
                    res = Some(allocation.clone());
                    break;
                }
            }
            IN_ALLOCATOR.set(IN_ALLOCATOR.get() - 1);
            res
        }
    }
}
