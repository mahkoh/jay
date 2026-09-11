use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::utils::clonecell::CloneCell;
use crate::wire::ZwpLinuxDmabufFeedbackV1Id;
use crate::wire::zwp_linux_dmabuf_feedback_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::ops::DerefMut;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;

pub struct TestDmabufFeedback {
    pub feedback: TEEH<Feedback>,
    format_table: CloneCell<Option<Rc<OwnedFd>>>,
    format_table_size: Cell<usize>,
    pending_feedback: RefCell<PendingFeedback>,
}

#[derive(Default)]
pub struct PendingFeedback {
    main_device: c::dev_t,
    tranches: Vec<Tranche>,
    pending_tranche: Tranche,
}

pub struct Feedback {
    _main_device: c::dev_t,
    pub tranches: Vec<Tranche>,
}

#[derive(Default)]
pub struct Tranche {
    pub target_device: c::dev_t,
    formats: Vec<usize>,
    pub flags: u32,
}

impl TestDmabufFeedback {
    fn new(client: &Rc<Client>, id: ZwpLinuxDmabufFeedbackV1Id) -> Rc<Self> {
        let slf = Rc::new(Self {
            feedback: Default::default(),
            format_table: Default::default(),
            format_table_size: Default::default(),
            pending_feedback: Default::default(),
        });
        client.set_synthetic_event_handler(id, &slf);
        slf
    }
}

synthetic_event_handler!(TestDmabufFeedback);

impl ZwpLinuxDmabufFeedbackV1EventHandler for TestDmabufFeedback {
    type Error = TestErrorError;

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pending = mem::take(self.pending_feedback.borrow_mut().deref_mut());
        self.feedback.push(Feedback {
            _main_device: pending.main_device,
            tranches: pending.tranches,
        });
        Ok(())
    }

    fn format_table(&self, ev: FormatTable, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.format_table.set(Some(ev.fd));
        self.format_table_size.set(ev.size as _);
        Ok(())
    }

    fn main_device(&self, ev: MainDevice, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.main_device = ev.device;
        Ok(())
    }

    fn tranche_done(&self, _ev: TrancheDone, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending
            .tranches
            .push(mem::take(&mut pending.pending_tranche));
        Ok(())
    }

    fn tranche_target_device(
        &self,
        ev: TrancheTargetDevice,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.pending_tranche.target_device = ev.device;
        Ok(())
    }

    fn tranche_formats(&self, ev: TrancheFormats<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.pending_tranche.formats = ev.indices.iter().copied().map(|v| v as usize).collect();
        Ok(())
    }

    fn tranche_flags(&self, ev: TrancheFlags, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pending = &mut *self.pending_feedback.borrow_mut();
        pending.pending_tranche.flags = ev.flags;
        Ok(())
    }
}

impl TestClient {
    #[expect(unused)]
    pub fn get_default_dmabuf_feedback(&self) -> Rc<TestDmabufFeedback> {
        let id = self.client.send_zwp_linux_dmabuf_v1_get_default_feedback();
        TestDmabufFeedback::new(&self.client, id)
    }

    pub fn get_surface_dmabuf_feedback(&self, surface: &TestSurface) -> Rc<TestDmabufFeedback> {
        let id = self
            .client
            .send_zwp_linux_dmabuf_v1_get_surface_feedback(surface.id);
        TestDmabufFeedback::new(&self.client, id)
    }
}
