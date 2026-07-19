use crate::sm::SessionManager;
use crate::sm::ToplevelSession;
use crate::sm::sm_jobs::SmDbStateHolder;
use crate::sm::sm_jobs::SmJob;
use crate::sm::sm_jobs::SmPending;
use crate::sqlite::SqliteCtx;
use crate::sqlite::SqliteJob;
use crate::sqlite::SqliteWork;
use crate::utils::cell_ext::CellExt;
use crate::utils::stack::Stack;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;

pub struct ToplevelRoundtripJob {
    pending: Weak<SmPending<Self>>,
    work: ToplevelRoundtripWork,
}

struct ToplevelRoundtripWork;

impl SmJob for ToplevelRoundtripJob {
    type Cb = ToplevelSession;

    fn new(_db_state: Arc<SmDbStateHolder>, pending: Weak<SmPending<Self>>) -> Self {
        Self {
            pending,
            work: ToplevelRoundtripWork,
        }
    }

    fn stack(sm: &SessionManager) -> &Stack<Rc<SmPending<Self>>> {
        &sm.toplevel_roundtrip_jobs
    }
}

impl SqliteJob for ToplevelRoundtripJob {
    fn work(&mut self) -> &mut dyn SqliteWork {
        &mut self.work
    }

    fn completed(self: Box<Self>) {
        let Some(pending) = self.pending.upgrade() else {
            return;
        };
        pending.job.set(Some(self));
        let Some(cb) = pending.cb.take() else {
            return;
        };
        cb.job.take();
        if cb.session.id.is_some() {
            cb.schedule_job(true);
        }
    }
}

impl SqliteWork for ToplevelRoundtripWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        let _ = ctx;
    }
}
