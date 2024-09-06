use {
    crate::{it::test_error::TestResult, utils::clonecell::CloneCell},
    isnt::std_1::collections::IsntVecDequeExt,
    std::{cell::RefCell, collections::VecDeque, rc::Rc},
};

pub struct TestExpectedEvent<T> {
    data: Rc<TestExpectedEventData<T>>,
    holder: Rc<TestExpectedEventHolder<T>>,
}

impl<T> TestExpectedEvent<T> {
    pub fn next(&self) -> TestResult<T> {
        match self.data.events.borrow_mut().pop_front() {
            Some(t) => Ok(t),
            _ => bail!("No event occurred"),
        }
    }

    pub fn last(&self) -> TestResult<T> {
        match self.data.events.borrow_mut().pop_back() {
            Some(t) => Ok(t),
            _ => bail!("No event occurred"),
        }
    }

    pub fn none(&self) -> TestResult {
        if self.data.events.borrow_mut().is_not_empty() {
            bail!("There are unexpected events");
        }
        Ok(())
    }
}

pub struct TestExpectedEventHolder<T> {
    data: CloneCell<Option<Rc<TestExpectedEventData<T>>>>,
}

pub type TEEH<T> = Rc<TestExpectedEventHolder<T>>;

impl<T> Default for TestExpectedEventHolder<T> {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl<T> TestExpectedEventHolder<T> {
    pub fn expect(self: &Rc<Self>) -> TestResult<TestExpectedEvent<T>> {
        if self.data.is_some() {
            bail!("There is already an expected event data");
        }
        let data = Rc::new(TestExpectedEventData {
            events: Default::default(),
        });
        self.data.set(Some(data.clone()));
        Ok(TestExpectedEvent {
            data,
            holder: self.clone(),
        })
    }

    pub fn push(&self, t: T) {
        if let Some(data) = self.data.get() {
            data.events.borrow_mut().push_back(t);
        }
    }
}

struct TestExpectedEventData<T> {
    events: RefCell<VecDeque<T>>,
}

impl<T> Drop for TestExpectedEvent<T> {
    fn drop(&mut self) {
        self.holder.data.set(None);
    }
}
