use {
    crate::{
        it::{
            test_backend::TestBackend,
            test_config::{with_test_config, TestConfig},
            testrun::TestRun,
            tests::TestCase,
        },
        utils::errorfmt::ErrorFmt,
    },
    ahash::AHashMap,
    futures_util::{future, future::Either},
    isnt::std_1::collections::IsntHashMapExt,
    log::Level,
    std::{
        cell::{Cell, RefCell},
        future::pending,
        pin::Pin,
        rc::Rc,
        time::SystemTime,
    },
    uapi::c,
};

#[macro_use]
mod test_error;
#[macro_use]
mod test_object;
pub mod test_backend;
mod test_client;
pub mod test_config;
mod test_ifs;
mod test_logger;
mod test_mem;
mod test_transport;
mod test_utils;
mod testrun;
mod tests;

pub fn run_tests() {
    test_logger::install();
    test_logger::set_level(Level::Trace);
    let it_run = ItRun {
        path: format!(
            "{}/testruns/{}",
            env!("CARGO_MANIFEST_DIR"),
            humantime::format_rfc3339_millis(SystemTime::now())
        ),
        failed: Default::default(),
    };
    for test in tests::tests() {
        with_test_config(|cfg| {
            run_test(&it_run, test, cfg);
        })
    }
    let failed = it_run.failed.borrow_mut();
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
    failed: RefCell<AHashMap<&'static str, Vec<String>>>,
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
            dir: dir.clone(),
            cfg: cfg.clone(),
        });
        let errors = errors2.clone();
        Box::new(async move {
            let future: Pin<_> = test.run(testrun.clone()).into();
            let timeout = state.eng.timeout(5000).unwrap();
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
            state.el.stop();
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
        it_run.failed.borrow_mut().insert(test.name(), errors);
    }
    test_logger::unset_file();
}
