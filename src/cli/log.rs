use {
    crate::{
        cli::{GlobalArgs, LogArgs},
        tools::tool_client::{with_tool_client, Handle, ToolClient},
        utils::errorfmt::ErrorFmt,
        wire::{jay_compositor, jay_log_file},
    },
    bstr::{BString, ByteSlice},
    jay_compositor::GetLogFile,
    jay_log_file::Path,
    std::{
        cell::RefCell,
        ops::Deref,
        os::unix::process::CommandExt,
        process::{self, Command},
        rc::Rc,
    },
};

pub fn main(global: GlobalArgs, args: LogArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let logger = Rc::new(Log {
            tc: tc.clone(),
            path: RefCell::new(None),
            args,
        });
        run(logger).await;
    });
}

struct Log {
    tc: Rc<ToolClient>,
    path: RefCell<Option<BString>>,
    args: LogArgs,
}

async fn run(log: Rc<Log>) {
    let tc = &log.tc;
    let comp = tc.jay_compositor().await;
    let log_file = tc.id();
    tc.send(GetLogFile {
        self_id: comp,
        id: log_file,
    });
    Path::handle(tc, log_file, log.clone(), |log, path| {
        *log.path.borrow_mut() = Some(path.path.to_vec().into());
    });
    tc.round_trip().await;
    let path = log.path.borrow_mut();
    let path = match path.deref() {
        Some(p) => p,
        _ => fatal!("Server did not send the path of the log file"),
    };
    if log.args.path {
        println!("{}", path);
        process::exit(0);
    }
    let mut command = Command::new("less");
    if log.args.pager_end {
        command.arg("+G");
    }
    if log.args.follow {
        command.arg("+F");
    } else {
        command.arg("-S");
    }
    command.arg(path.as_bytes().to_os_str().unwrap());
    let err = command.exec();
    fatal!("Could not spawn `less`: {}", ErrorFmt(err));
}
