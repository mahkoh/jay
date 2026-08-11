use crate::State;
use crate::config::context::Context;
use crate::config::parsers::trigger::TomlTrigger;
use crate::config::parsers::trigger::Trigger;
use crate::weak_once_cell::WeakOnceCell;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;

#[derive(Debug)]
pub struct CounterSlot {
    value: Rc<Cell<i64>>,
    raw_triggers: RefCell<Vec<Weak<Trigger>>>,
    counter: WeakOnceCell<Counter>,
}

#[derive(Debug)]
pub struct Counter {
    value: Rc<Cell<i64>>,
    triggers: Vec<Weak<TomlTrigger>>,
}

impl Context<'_, '_> {
    pub fn get_counter_slot(&self, name: &str) -> Rc<CounterSlot> {
        let map = &mut *self.counters.borrow_mut();
        if let Some(slot) = map.get(name) {
            return slot.clone();
        }
        let slot = Rc::new(CounterSlot {
            value: Default::default(),
            raw_triggers: Default::default(),
            counter: Default::default(),
        });
        map.insert(name.to_string(), slot.clone());
        slot
    }
}

impl CounterSlot {
    pub fn value(&self) -> &Rc<Cell<i64>> {
        &self.value
    }

    pub fn add_trigger(&self, trigger: &Rc<Trigger>) {
        self.raw_triggers.borrow_mut().push(Rc::downgrade(trigger));
    }

    pub fn build(&self, state: &Rc<State>) -> Weak<Counter> {
        self.counter.get_or_init(
            || Counter {
                value: self.value.clone(),
                triggers: self
                    .raw_triggers
                    .borrow()
                    .iter()
                    .filter_map(|t| t.upgrade())
                    .map(|t| t.build(state))
                    .collect(),
            },
            |rc| state.persistent.counters.borrow_mut().push(rc),
        )
    }
}

impl Counter {
    pub fn adjust(&self, v: i64) {
        let v = self.value.get().wrapping_add(v);
        self.set(v);
    }

    pub fn set(&self, v: i64) {
        if self.value.replace(v) == v {
            return;
        }
        for t in &*self.triggers {
            if let Some(t) = t.upgrade() {
                t.check_active();
            }
        }
    }
}
