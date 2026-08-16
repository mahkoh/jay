use crate::State;
use crate::config::Action;
use crate::config::context::Context;
use crate::config::counter::CounterSlot;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::arr;
use crate::config::extractor::int;
use crate::config::extractor::n32;
use crate::config::extractor::opt;
use crate::config::extractor::tbl;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::action::ActionParser;
use crate::config::parsers::action::ActionParserError;
use crate::config::spanned::SpannedErrorExt;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use crate::weak_once_cell::WeakOnceCell;
use ahash::AHashMap;
use derivative::Derivative;
use indexmap::IndexMap;
use run_on_drop::on_drop;
use std::cell::Cell;
use std::rc::Rc;
use std::rc::Weak;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TriggerParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("Could not parse the action")]
    Action(ActionParserError),
    #[error("Could not parse the latch")]
    Latch(ActionParserError),
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct TomlTrigger {
    max_depth: u64,
    match_: Rc<TriggerMatch>,
    active: Cell<bool>,
    #[derivative(Debug = "ignore")]
    action: Option<Box<dyn Fn()>>,
    #[derivative(Debug = "ignore")]
    latch: Option<Box<dyn Fn()>>,
}

#[derive(Debug)]
struct TriggerCounter {
    counter: Rc<Cell<i64>>,
    eq: Option<i64>,
    ne: Option<i64>,
    gt: Option<i64>,
    ge: Option<i64>,
    lt: Option<i64>,
    le: Option<i64>,
}

#[derive(Debug)]
struct TriggerExactly {
    num: usize,
    list: Vec<TriggerMatch>,
}

#[derive(Debug)]
struct TriggerMatch {
    all: Option<Vec<TriggerMatch>>,
    any: Option<Vec<TriggerMatch>>,
    not: Option<Box<TriggerMatch>>,
    exactly: Option<TriggerExactly>,
    counter: Option<Vec<TriggerCounter>>,
}

#[derive(Debug)]
pub struct Trigger {
    trigger: WeakOnceCell<TomlTrigger>,
    match_: Rc<TriggerMatch>,
    action: Option<Action>,
    latch: Option<Action>,
}

impl Trigger {
    pub fn build(&self, state: &Rc<State>) -> Weak<TomlTrigger> {
        self.trigger.get_or_init(
            || TomlTrigger {
                max_depth: state.max_trigger_depth,
                match_: self.match_.clone(),
                active: Default::default(),
                action: self.action.clone().map(|a| a.into_fn(state)),
                latch: self.latch.clone().map(|a| a.into_fn(state)),
            },
            |rc| state.persistent.triggers.borrow_mut().push(rc),
        )
    }
}

impl TriggerExactly {
    fn active(&self) -> bool {
        let mut n = 0;
        for c in &self.list {
            if c.active() {
                n += 1;
            }
        }
        n == self.num
    }
}

impl TriggerCounter {
    fn active(&self) -> bool {
        let v = self.counter.get();
        macro_rules! check {
            ($field:ident, $tt:tt) => {
                if let Some(f) = self.$field
                    && !(v $tt f)
                {
                    return false;
                }
            };
        }
        check!(eq, ==);
        check!(ne, !=);
        check!(gt, >);
        check!(ge, >=);
        check!(lt, <);
        check!(le, <=);
        true
    }
}

impl TriggerMatch {
    fn active(&self) -> bool {
        if let Some(v) = &self.all
            && !v.iter().all(|v| v.active())
        {
            return false;
        }
        if let Some(v) = &self.any
            && !v.iter().any(|v| v.active())
        {
            return false;
        }
        if let Some(v) = &self.not
            && v.active()
        {
            return false;
        }
        if let Some(v) = &self.exactly
            && !v.active()
        {
            return false;
        }
        if let Some(v) = &self.counter
            && !v.iter().all(|v| v.active())
        {
            return false;
        }
        true
    }
}

impl TomlTrigger {
    pub fn check_active(&self) {
        let old = self.active.get();
        let new = self.match_.active();
        if new == old {
            return;
        }
        thread_local! {
            static DEPTH: Cell<u64> = const { Cell::new(0) };
        }
        let depth = DEPTH.get();
        if depth >= self.max_depth {
            log::error!("Maximum trigger depth reached");
            return;
        }
        DEPTH.set(depth + 1);
        let _reset = on_drop(|| DEPTH.set(depth));
        self.active.set(new);
        if new {
            if let Some(v) = &self.action {
                v();
            }
        } else {
            if let Some(v) = &self.latch {
                v();
            }
        }
    }
}

pub struct TriggersParser<'a, 'b, 'c>(pub &'a Context<'b, 'c>);

impl Parser for TriggersParser<'_, '_, '_> {
    type Value = Vec<Rc<Trigger>>;
    type Error = TriggerParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for v in array {
            match v.parse(&mut TriggerParser(self.0)) {
                Ok(t) => res.push(t),
                Err(e) => {
                    log::warn!("Could not parse trigger: {}", self.0.error(e));
                }
            }
        }
        Ok(res)
    }
}

struct TriggerParser<'a, 'b, 'c>(&'a Context<'b, 'c>);

impl Parser for TriggerParser<'_, '_, '_> {
    type Value = Rc<Trigger>;
    type Error = TriggerParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (
            match_val, //
            action_val,
            latch_val,
        ) = ext.extract((
            val("match"), //
            opt(val("action")),
            opt(val("latch")),
        ))?;
        let mut counters = AHashMap::new();
        let match_ = match_val.parse(&mut TriggerMatchParser {
            cx: self.0,
            counters: &mut counters,
        })?;
        let mut action = None;
        if let Some(val) = action_val {
            action = Some(
                val.parse(&mut ActionParser(self.0))
                    .map_spanned_err(TriggerParserError::Action)?,
            );
        }
        let mut latch = None;
        if let Some(val) = latch_val {
            latch = Some(
                val.parse(&mut ActionParser(self.0))
                    .map_spanned_err(TriggerParserError::Latch)?,
            );
        }
        let trigger = Rc::new(Trigger {
            trigger: Default::default(),
            match_: Rc::new(match_),
            action,
            latch,
        });
        for counter in counters.values() {
            counter.add_trigger(&trigger);
        }
        Ok(trigger)
    }
}

struct TriggerMatchParser<'a, 'b, 'c, 'd> {
    cx: &'a Context<'b, 'c>,
    counters: &'d mut AHashMap<String, Rc<CounterSlot>>,
}

impl Parser for TriggerMatchParser<'_, '_, '_, '_> {
    type Value = TriggerMatch;
    type Error = TriggerParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table, DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for v in array {
            res.push(v.parse(self)?);
        }
        Ok(TriggerMatch {
            all: Default::default(),
            any: Some(res),
            not: Default::default(),
            exactly: Default::default(),
            counter: Default::default(),
        })
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (
            all_val, //
            any_val,
            not_val,
            exactly_val,
            counter_val,
        ) = ext.extract((
            opt(arr("all")),
            opt(arr("any")),
            opt(val("not")),
            opt(val("exactly")),
            opt(tbl("counter")),
        ))?;
        let mut all = None::<Vec<_>>;
        if let Some(v) = &all_val {
            let l = all.get_or_insert_default();
            for v in v.value {
                l.push(v.parse(self)?);
            }
        }
        let mut any = None::<Vec<_>>;
        if let Some(v) = &any_val {
            let l = any.get_or_insert_default();
            for v in v.value {
                l.push(v.parse(self)?);
            }
        }
        let mut not = None;
        if let Some(v) = &not_val {
            not = Some(Box::new(v.parse(self)?));
        }
        let mut exactly = None;
        if let Some(v) = &exactly_val {
            exactly = Some(v.parse(&mut TriggerExactlyParser {
                cx: self.cx,
                counters: self.counters,
            })?);
        }
        let mut counter = None::<Vec<TriggerCounter>>;
        if let Some(v) = &counter_val {
            let counter = counter.get_or_insert_default();
            for (k, v) in v.value {
                let slot = self.cx.get_counter_slot(&k.value);
                let tc = v.parse(&mut TriggerCounterParser {
                    cx: self.cx,
                    counter: slot.value(),
                })?;
                counter.push(tc);
                self.counters.insert(k.value.clone(), slot);
            }
        }
        Ok(TriggerMatch {
            all,
            any,
            not,
            exactly,
            counter,
        })
    }
}

struct TriggerExactlyParser<'a, 'b, 'c, 'd> {
    cx: &'a Context<'b, 'c>,
    counters: &'d mut AHashMap<String, Rc<CounterSlot>>,
}

impl Parser for TriggerExactlyParser<'_, '_, '_, '_> {
    type Value = TriggerExactly;
    type Error = TriggerParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (
            num, //
            list_val,
        ) = ext.extract((
            n32("num"), //
            arr("list"),
        ))?;
        let mut list = vec![];
        for v in list_val.value {
            list.push(v.parse(&mut TriggerMatchParser {
                cx: self.cx,
                counters: self.counters,
            })?);
        }
        Ok(TriggerExactly {
            num: num.value as usize,
            list,
        })
    }
}

struct TriggerCounterParser<'a, 'b, 'c, 'd> {
    cx: &'a Context<'b, 'c>,
    counter: &'d Rc<Cell<i64>>,
}

impl Parser for TriggerCounterParser<'_, '_, '_, '_> {
    type Value = TriggerCounter;
    type Error = TriggerParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (
            eq, //
            ne,
            gt,
            ge,
            lt,
            le,
        ) = ext.extract((
            opt(int("eq")),
            opt(int("ne")),
            opt(int("gt")),
            opt(int("ge")),
            opt(int("lt")),
            opt(int("le")),
        ))?;
        Ok(TriggerCounter {
            counter: self.counter.clone(),
            eq: eq.despan(),
            ne: ne.despan(),
            gt: gt.despan(),
            ge: ge.despan(),
            lt: lt.despan(),
            le: le.despan(),
        })
    }
}
