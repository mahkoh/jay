use {
    crate::{
        async_engine::Phase,
        it::{
            test_backend::TestBackend,
            test_config::{TestConfig, with_test_config},
            testrun::TestRun,
            tests::TestCase,
        },
        leaks,
        utils::{errorfmt::ErrorFmt, num_cpus::num_cpus},
    },
    ahash::AHashMap,
    futures_util::{future, future::Either},
    isnt::std_1::collections::IsntHashMapExt,
    log::Level,
    parking_lot::Mutex,
    std::{
        cell::Cell, collections::VecDeque, future::pending, pin::Pin, rc::Rc, sync::Arc,
        time::SystemTime,
    },
    uapi::c,
};

#[macro_use]
mod test_error;
#[macro_use]
mod test_object;
#[macro_use]
mod test_macros;
pub mod test_backend;
mod test_client;
pub mod test_config;
mod test_gfx_api;
mod test_ifs;
mod test_logger;
mod test_mem;
mod test_transport;
mod test_utils;
mod testrun;
mod tests;

const SINGLE_THREAD: bool = false;

pub fn run_tests() {
    run_tests_(tests::tests());
}

fn run_tests_(tests: Vec<&'static dyn TestCase>) {
    leaks::init();
    test_logger::install();
    test_logger::set_level(Level::Trace);
    let it_run = Arc::new(ItRun {
        path: format!(
            "{}/testruns/{}",
            env!("CARGO_MANIFEST_DIR"),
            humantime::format_rfc3339_millis(SystemTime::now())
        ),
        failed: Default::default(),
    });
    if SINGLE_THREAD {
        for test in tests {
            with_test_config(|cfg| {
                run_test(&it_run, test, cfg);
            })
        }
    } else {
        let queue = Arc::new(Mutex::new(VecDeque::from_iter(tests)));
        let mut threads = vec![];
        let num_cpus = match num_cpus() {
            Ok(n) => n,
            Err(e) => fatal!("Could not determine the number of cpus: {}", ErrorFmt(e)),
        };
        log::info!("Running {} tests in parallel", num_cpus);
        for _ in 0..num_cpus {
            let queue = queue.clone();
            let it_run = it_run.clone();
            threads.push(std::thread::spawn(move || {
                loop {
                    let test = match queue.lock().pop_front() {
                        Some(t) => t,
                        _ => break,
                    };
                    with_test_config(|cfg| {
                        run_test(&it_run, test, cfg);
                    })
                }
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
    }
    let failed = it_run.failed.lock();
    if failed.is_not_empty() {
        let mut failed: Vec<_> = failed.iter().collect();
        failed.sort_by_key(|f| f.0);
        log::error!("The following tests failed:");
        for (name, errors) in failed {
            log::error!("    {}:", name);
            for error in errors {
                log::error!("        {}", error);
            }
        }
        fatal!("Some tests failed");
    }
}

struct ItRun {
    path: String,
    failed: Mutex<AHashMap<&'static str, Vec<String>>>,
}

fn run_test(it_run: &ItRun, test: &'static dyn TestCase, cfg: Rc<TestConfig>) {
    log::info!("Running {}", test.name());
    let dir = format!("{}/{}", it_run.path, test.name());
    std::fs::create_dir_all(&dir).unwrap();
    let log_path = format!("{}/log", dir);
    let log_file = Rc::new(uapi::open(log_path.as_str(), c::O_WRONLY | c::O_CREAT, 0o644).unwrap());
    test_logger::set_file(log_file);
    let errors = Rc::new(Cell::new(Vec::new()));
    let errors2 = errors.clone();
    let res = crate::compositor::start_compositor_for_test(Box::new(move |state| {
        let state = state.clone();
        let server_addr = {
            let mut addr: c::sockaddr_un = uapi::pod_zeroed();
            addr.sun_family = c::AF_UNIX as _;
            let acceptor = state.acceptor.get().unwrap();
            let path = acceptor.secure_path();
            let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
            sun_path[..path.len()].copy_from_slice(path.as_bytes());
            sun_path[path.len()] = 0;
            addr
        };
        let backend: Rc<TestBackend> = state.backend.get().into_any().downcast().unwrap();
        let testrun = Rc::new(TestRun {
            state: state.clone(),
            backend,
            errors: Default::default(),
            server_addr,
            out_dir: dir.clone(),
            in_dir: format!("{}/{}", env!("CARGO_MANIFEST_DIR"), test.dir()),
            cfg: cfg.clone(),
        });
        let errors = errors2.clone();
        Box::new(async move {
            let future: Pin<_> = test.run(testrun.clone()).into();
            let future = state.eng.spawn2("testrun", Phase::Present, future);
            let timeout = state.wheel.timeout(500000);
            match future::select(future, timeout).await {
                Either::Left((Ok(..), _)) => {}
                Either::Left((Err(e), _)) => {
                    testrun.errors.push(e.to_string());
                }
                Either::Right(..) => {
                    testrun.errors.push("Test timed out".to_string());
                }
            }
            errors.set(testrun.errors.take());
            state.ring.stop();
            pending().await
        })
    }));
    let mut errors = errors.take();
    if let Err(e) = res {
        errors.push(format!("The compositor failed: {}", ErrorFmt(e)));
    }
    if errors.len() > 0 {
        log::error!("The following errors occurred:");
        for e in &errors {
            log::error!("    {}", e);
        }
        it_run.failed.lock().insert(test.name(), errors);
    }
    test_logger::unset_file();
}
