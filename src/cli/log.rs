use crate::cli::{GlobalArgs, LogArgs};
use crate::tools::tool_client::{Handle, ToolClient};
use crate::utils::errorfmt::ErrorFmt;
use crate::wire::{jay_compositor, jay_log_file};
use bstr::{BString, ByteSlice};
use jay_compositor::GetLogFile;
use jay_log_file::Path;
use std::cell::RefCell;
use std::ops::Deref;
use std::os::unix::process::CommandExt;
use std::process;
use std::process::Command;
use std::rc::Rc;

pub fn main(global: GlobalArgs, args: LogArgs) {
    let tc = ToolClient::new(global.log_level.into());
    let logger = Rc::new(Log {
        tc: tc.clone(),
        path: RefCell::new(None),
        args,
    });
    tc.run(run(logger));
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
