use {
    crate::{async_engine::AsyncEngine, io_uring::IoUring, logger, pipewire::pw_con::PwCon},
    log::Level,
    pipewire::{
        prelude::ListenerBuilderT,
        properties,
        spa::Direction,
        stream::{Stream, StreamFlags},
        sys::pw_log_set_level,
        Context, MainLoop,
    },
};

pub fn run() {
    run1();
    // run2();
}

pub fn run1() {
    let logger = logger::Logger::install_stderr(Level::Trace);
    let ae = AsyncEngine::new();
    let ring = IoUring::new(&ae, 32).unwrap();
    let con = PwCon::new(&ae, &ring).unwrap();
    ring.run().unwrap();
}

pub fn run2() {
    unsafe {
        pw_log_set_level(4);
    }
    let ml = MainLoop::new().unwrap();
    let ctx = Context::new(&ml).unwrap();
    unsafe {
        pw_log_set_level(4);
    }
    let core = ctx.connect(None).unwrap();
    let mut stream = Stream::<()>::new(
        &core,
        "hurr-durr",
        properties! {
            "media.class" => "Video/Source",
        },
    )
    .unwrap();

    let listener = stream
        .add_local_listener()
        .state_changed(|a, b| {
            dbg!(a);
            dbg!(b);
        })
        .param_changed(|a, b, c| {
            dbg!(a);
            dbg!(b);
            dbg!(c);
        })
        .add_buffer(|a| {
            dbg!(a);
        })
        .remove_buffer(|a| {
            dbg!(a);
        })
        .register()
        .unwrap();
    stream
        .connect(
            Direction::Output,
            None,
            StreamFlags::DRIVER | StreamFlags::ALLOC_BUFFERS,
            &mut [],
        )
        .unwrap();
    ml.run();
    loop {}
}
