use {
    crate::{
        it::{
            test_error::TestResult, test_object::TestObject, test_transport::TestTransport,
            test_utils::test_expected_event::TEEH, testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{ZwpLinuxDmabufFeedbackV1Id, zwp_linux_dmabuf_feedback_v1::*},
    },
    std::{
        cell::{Cell, RefCell},
        mem,
        ops::DerefMut,
        rc::Rc,
    },
    uapi::{OwnedFd, c},
};

pub struct TestDmabufFeedback {
    pub id: ZwpLinuxDmabufFeedbackV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub feedback: TEEH<Feedback>,
    pub pending_feedback: RefCell<PendingFeedback>,
}

#[derive(Default)]
pub struct PendingFeedback {
    pub format_table: Option<Rc<OwnedFd>>,
    pub format_table_size: usize,
    pub main_device: c::dev_t,
    pub tranches: Vec<Tranche>,
    pub pending_tranche: Tranche,
}

pub struct Feedback {
    pub _format_table: Rc<OwnedFd>,
    pub _format_table_size: usize,
    pub _main_device: c::dev_t,
    pub tranches: Vec<Tranche>,
}

#[derive(Default)]
pub struct Tranche {
    pub target_device: c::dev_t,
    pub formats: Vec<usize>,
    pub flags: u32,
}

impl TestDmabufFeedback {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
            feedback: Rc::new(Default::default()),
            pending_feedback: RefCell::new(Default::default()),
        }
    }

    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    fn handle_done(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Done::parse_full(parser)?;
        let mut pending = mem::take(self.pending_feedback.borrow_mut().deref_mut());
        self.feedback.push(Feedback {
            _format_table: match pending.format_table.take() {
                None => bail!("compositor did not send format table"),
                Some(ft) => ft,
            },
            _format_table_size: pending.format_table_size,
            _main_device: pending.main_device,
            tranches: pending.tranches,
        });
        Ok(())
    }

    fn handle_format_table(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = FormatTable::parse_full(parser)?;
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.format_table = Some(ev.fd);
        pending.format_table_size = ev.size as _;
        Ok(())
    }

    fn handle_main_device(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = MainDevice::parse_full(parser)?;
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.main_device = ev.device;
        Ok(())
    }

    fn handle_tranche_done(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = TrancheDone::parse_full(parser)?;
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending
            .tranches
            .push(mem::take(&mut pending.pending_tranche));
        Ok(())
    }

    fn handle_tranche_target_device(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = TrancheTargetDevice::parse_full(parser)?;
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.pending_tranche.target_device = ev.device;
        Ok(())
    }

    fn handle_tranche_formats(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = TrancheFormats::parse_full(parser)?;
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.pending_tranche.formats = ev.indices.iter().copied().map(|v| v as usize).collect();
        Ok(())
    }

    fn handle_tranche_flags(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = TrancheFlags::parse_full(parser)?;
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.pending_tranche.flags = ev.flags;
        Ok(())
    }
}

impl Drop for TestDmabufFeedback {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestDmabufFeedback, ZwpLinuxDmabufFeedbackV1;

    DONE => handle_done,
    FORMAT_TABLE => handle_format_table,
    MAIN_DEVICE => handle_main_device,
    TRANCHE_DONE => handle_tranche_done,
    TRANCHE_TARGET_DEVICE => handle_tranche_target_device,
    TRANCHE_FORMATS => handle_tranche_formats,
    TRANCHE_FLAGS => handle_tranche_flags,
}

impl TestObject for TestDmabufFeedback {}
