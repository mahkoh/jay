use {
    crate::{
        State,
        config::{ClientMatch, ClientRule, GenericMatch, WindowMatch, WindowRule},
    },
    ahash::{AHashMap, AHashSet},
    jay_config::{
        client::{ClientCriterion, ClientMatcher},
        window::{WindowCriterion, WindowMatcher},
    },
    std::{mem::ManuallyDrop, rc::Rc},
};

impl State {
    pub fn create_rules<R>(self: &Rc<Self>, rules: &[R]) -> (Vec<MatcherTemp<R>>, RuleMapper<R>)
    where
        R: Rule,
    {
        let mut names = AHashMap::new();
        for (idx, rule) in rules.iter().enumerate() {
            if let Some(name) = rule.name() {
                names.insert(name.to_string(), idx);
            }
        }
        let mut mapper = RuleMapper {
            state: self.clone(),
            names,
            pending: Default::default(),
            mapped: Default::default(),
        };
        let mut matchers = vec![];
        for idx in 0..rules.len() {
            if let Some(matcher) = mapper.map_rule(rules, idx) {
                matchers.push(MatcherTemp(matcher));
            }
        }
        (matchers, mapper)
    }
}

pub trait Rule: Sized + 'static {
    type Match;
    type Matcher: Copy + 'static;
    type Criterion<'a>;

    const NAME_UPPER: &str;
    const NAME_LOWER: &str;

    fn name(&self) -> Option<&str>;
    fn match_(&self) -> &Self::Match;
    fn generic(m: &Self::Match) -> &GenericMatch<Self::Match>;
    fn map_custom(
        state: &Rc<State>,
        all: &mut Vec<MatcherTemp<Self>>,
        match_: &Self::Match,
    ) -> Option<()>;
    fn create(c: Self::Criterion<'_>) -> Self::Matcher;
    fn destroy(m: Self::Matcher);
    fn bind(&self, state: &Rc<State>, matcher: Self::Matcher);

    fn gen_matcher(m: Self::Matcher) -> Self::Criterion<'static>;
    fn gen_not<'a, 'b: 'a>(m: &'a Self::Criterion<'b>) -> Self::Criterion<'a>;
    fn gen_all<'a, 'b: 'a>(m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a>;
    fn gen_any<'a, 'b: 'a>(m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a>;
    fn gen_exactly<'a, 'b: 'a>(n: usize, m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a>;
}

impl Rule for ClientRule {
    type Match = ClientMatch;
    type Matcher = ClientMatcher;
    type Criterion<'a> = ClientCriterion<'a>;

    const NAME_UPPER: &str = "Client";
    const NAME_LOWER: &str = "client";

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn match_(&self) -> &Self::Match {
        &self.match_
    }

    fn generic(m: &Self::Match) -> &GenericMatch<Self::Match> {
        &m.generic
    }

    fn map_custom(
        _state: &Rc<State>,
        all: &mut Vec<MatcherTemp<Self>>,
        match_: &Self::Match,
    ) -> Option<()> {
        let m = |c: ClientCriterion<'_>| MatcherTemp(c.to_matcher());
        macro_rules! value_ref {
            ($ty:ident, $field:ident) => {
                if let Some(value) = &match_.$field {
                    all.push(m(ClientCriterion::$ty(value)));
                }
            };
        }
        macro_rules! value {
            ($ty:ident, $field:ident) => {
                if let Some(value) = match_.$field {
                    all.push(m(ClientCriterion::$ty(value)));
                }
            };
        }
        macro_rules! bool {
            ($ty:ident, $field:ident) => {
                if let Some(value) = &match_.$field {
                    let crit = ClientCriterion::$ty;
                    let matcher = match value {
                        false => m(ClientCriterion::Not(&crit)),
                        true => m(crit),
                    };
                    all.push(matcher);
                }
            };
        }
        value_ref!(SandboxEngine, sandbox_engine);
        value_ref!(SandboxEngineRegex, sandbox_engine_regex);
        value_ref!(SandboxAppId, sandbox_app_id);
        value_ref!(SandboxAppIdRegex, sandbox_app_id_regex);
        value_ref!(SandboxInstanceId, sandbox_instance_id);
        value_ref!(SandboxInstanceIdRegex, sandbox_instance_id_regex);
        value_ref!(Comm, comm);
        value_ref!(CommRegex, comm_regex);
        value_ref!(Exe, exe);
        value_ref!(ExeRegex, exe_regex);
        value!(Uid, uid);
        value!(Pid, pid);
        bool!(Sandboxed, sandboxed);
        bool!(IsXwayland, is_xwayland);
        Some(())
    }

    fn create(c: Self::Criterion<'_>) -> Self::Matcher {
        c.to_matcher()
    }

    fn destroy(m: Self::Matcher) {
        m.destroy();
    }

    fn bind(&self, state: &Rc<State>, matcher: Self::Matcher) {
        let state = state.clone();
        macro_rules! latch {
            ($g:ident, $client:ident) => {
                let g = $g.clone();
                let state = state.clone();
                $client.latch(move || {
                    state.with_client($client.client(), true, || g());
                });
            };
        }
        if let Some(action) = &self.action {
            let f = action.clone().into_fn(&state);
            if let Some(action) = &self.latch {
                let g = action.clone().into_rc_fn(&state);
                let state = state.clone();
                matcher.bind(move |client| {
                    state.with_client(client.client(), false, &f);
                    latch!(g, client);
                });
            } else {
                matcher.bind(move |client| {
                    state.with_client(client.client(), false, &f);
                });
            }
        } else {
            if let Some(action) = &self.latch {
                let g = action.clone().into_rc_fn(&state);
                matcher.bind(move |client| {
                    latch!(g, client);
                });
            }
        }
    }

    fn gen_matcher(m: Self::Matcher) -> Self::Criterion<'static> {
        ClientCriterion::Matcher(m)
    }

    fn gen_not<'a, 'b: 'a>(m: &'a Self::Criterion<'b>) -> Self::Criterion<'a> {
        ClientCriterion::Not(m)
    }

    fn gen_all<'a, 'b: 'a>(m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a> {
        ClientCriterion::All(m)
    }

    fn gen_any<'a, 'b: 'a>(m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a> {
        ClientCriterion::Any(m)
    }

    fn gen_exactly<'a, 'b: 'a>(n: usize, m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a> {
        ClientCriterion::Exactly(n, m)
    }
}

impl Rule for WindowRule {
    type Match = WindowMatch;
    type Matcher = WindowMatcher;
    type Criterion<'a> = WindowCriterion<'a>;

    const NAME_UPPER: &str = "Window";
    const NAME_LOWER: &str = "window";

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn match_(&self) -> &Self::Match {
        &self.match_
    }

    fn generic(m: &Self::Match) -> &GenericMatch<Self::Match> {
        &m.generic
    }

    fn map_custom(
        state: &Rc<State>,
        all: &mut Vec<MatcherTemp<Self>>,
        match_: &Self::Match,
    ) -> Option<()> {
        let m = |c: WindowCriterion<'_>| MatcherTemp(c.to_matcher());
        macro_rules! value {
            ($ty:ident, $field:ident) => {
                if let Some(value) = &match_.$field {
                    all.push(m(WindowCriterion::$ty(value)));
                }
            };
        }
        macro_rules! bool {
            ($ty:ident, $field:ident) => {
                if let Some(value) = &match_.$field {
                    let crit = WindowCriterion::$ty;
                    let matcher = match value {
                        false => m(WindowCriterion::Not(&crit)),
                        true => m(crit),
                    };
                    all.push(matcher);
                }
            };
        }
        if let Some(value) = &match_.types {
            all.push(m(WindowCriterion::Types(*value)));
        }
        if let Some(value) = &match_.client {
            let mut mapper = state.persistent.client_rule_mapper.borrow_mut();
            let mapper = mapper.as_mut()?;
            let matcher = mapper.map_temporary_match(&[], value)?;
            all.push(m(WindowCriterion::Client(&ClientCriterion::Matcher(
                matcher.0,
            ))));
        }
        value!(Title, title);
        value!(TitleRegex, title_regex);
        value!(AppId, app_id);
        value!(AppIdRegex, app_id_regex);
        value!(Tag, tag);
        value!(TagRegex, tag_regex);
        value!(XClass, x_class);
        value!(XClassRegex, x_class_regex);
        value!(XInstance, x_instance);
        value!(XInstanceRegex, x_instance_regex);
        value!(XRole, x_role);
        value!(XRoleRegex, x_role_regex);
        value!(WorkspaceName, workspace);
        value!(WorkspaceNameRegex, workspace_regex);
        bool!(Floating, floating);
        bool!(Visible, visible);
        bool!(Urgent, urgent);
        bool!(Fullscreen, fullscreen);
        bool!(JustMapped, just_mapped);
        if let Some(value) = match_.focused {
            let crit = WindowCriterion::Focus(state.persistent.seat);
            let matcher = match value {
                false => m(WindowCriterion::Not(&crit)),
                true => m(crit),
            };
            all.push(matcher);
        }
        Some(())
    }

    fn create(c: Self::Criterion<'_>) -> Self::Matcher {
        c.to_matcher()
    }

    fn destroy(m: Self::Matcher) {
        m.destroy();
    }

    fn bind(&self, state: &Rc<State>, matcher: Self::Matcher) {
        let state = state.clone();
        macro_rules! latch {
            ($g:ident, $client:ident, $win:ident) => {
                let g = $g.clone();
                let state = state.clone();
                $win.latch(move || {
                    state.with_client($client, true, || {
                        state.with_window(*$win, true, || g());
                    });
                });
            };
        }
        if let Some(action) = &self.action {
            let f = action.clone().into_fn(&state);
            if let Some(action) = &self.latch {
                let g = action.clone().into_rc_fn(&state);
                matcher.bind(move |win| {
                    let client = win.client();
                    state.with_client(client, false, || {
                        state.with_window(*win, false, &f);
                    });
                    latch!(g, client, win);
                });
            } else {
                matcher.bind(move |win| {
                    let client = win.client();
                    state.with_client(client, false, || {
                        state.with_window(*win, false, &f);
                    });
                });
            }
        } else {
            if let Some(action) = &self.latch {
                let g = action.clone().into_rc_fn(&state);
                matcher.bind(move |win| {
                    let client = win.client();
                    latch!(g, client, win);
                });
            }
        }
    }

    fn gen_matcher(m: Self::Matcher) -> Self::Criterion<'static> {
        WindowCriterion::Matcher(m)
    }

    fn gen_not<'a, 'b: 'a>(m: &'a Self::Criterion<'b>) -> Self::Criterion<'a> {
        WindowCriterion::Not(m)
    }

    fn gen_all<'a, 'b: 'a>(m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a> {
        WindowCriterion::All(m)
    }

    fn gen_any<'a, 'b: 'a>(m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a> {
        WindowCriterion::Any(m)
    }

    fn gen_exactly<'a, 'b: 'a>(n: usize, m: &'a [Self::Criterion<'b>]) -> Self::Criterion<'a> {
        WindowCriterion::Exactly(n, m)
    }
}

pub struct RuleMapper<R>
where
    R: Rule,
{
    state: Rc<State>,
    names: AHashMap<String, usize>,
    pending: AHashSet<usize>,
    mapped: AHashMap<usize, R::Matcher>,
}

pub struct MatcherTemp<R>(R::Matcher)
where
    R: Rule;

impl<R> Drop for MatcherTemp<R>
where
    R: Rule,
{
    fn drop(&mut self) {
        R::destroy(self.0);
    }
}

impl<R> RuleMapper<R>
where
    R: Rule,
{
    fn map_rule(&mut self, rules: &[R], idx: usize) -> Option<R::Matcher> {
        if let Some(matcher) = self.mapped.get(&idx) {
            return Some(*matcher);
        }
        if !self.pending.insert(idx) {
            if let Some(name) = rules.get(idx).and_then(|r| r.name()) {
                log::error!("{} rule `{name}` has a loop", R::NAME_UPPER);
            }
            return None;
        }
        let rule = &rules[idx];
        let matcher = self.map_match(rules, rule.match_())?;
        self.mapped.insert(idx, matcher);
        rule.bind(&self.state, matcher);
        Some(matcher)
    }

    fn map_temporary_match(&mut self, rules: &[R], matcher: &R::Match) -> Option<MatcherTemp<R>> {
        self.map_match(rules, matcher).map(MatcherTemp)
    }

    fn map_match(&mut self, rules: &[R], matcher: &R::Match) -> Option<R::Matcher> {
        let mut all = vec![];
        self.map_generic_match(rules, &mut all, R::generic(matcher))?;
        R::map_custom(&self.state, &mut all, matcher)?;
        if all.len() == 1 {
            return Some(ManuallyDrop::new(all.pop().unwrap()).0);
        }
        let all: Vec<_> = all.iter().map(|m| R::gen_matcher(m.0)).collect();
        Some(R::create(R::gen_all(&all)))
    }

    fn map_generic_match(
        &mut self,
        rules: &[R],
        all: &mut Vec<MatcherTemp<R>>,
        matcher: &GenericMatch<R::Match>,
    ) -> Option<()> {
        let m = |c: R::Criterion<'_>| MatcherTemp(R::create(c));
        if let Some(name) = &matcher.name {
            let Some(&idx) = self.names.get(&**name) else {
                log::error!("There is no {} rule named `{name}`", R::NAME_LOWER);
                return None;
            };
            let matcher = self.map_rule(rules, idx)?;
            all.push(m(R::gen_matcher(matcher)));
        }
        if let Some(not) = &matcher.not {
            let matcher = self.map_temporary_match(rules, not)?;
            all.push(m(R::gen_not(&R::gen_matcher(matcher.0))));
        }
        if let Some(list) = &matcher.all {
            for match_ in list {
                all.push(self.map_temporary_match(rules, match_)?);
            }
        }
        if let Some(list) = &matcher.any {
            let mut any = vec![];
            for match_ in list {
                any.push(self.map_temporary_match(rules, match_)?);
            }
            let any: Vec<_> = any.iter().map(|m| R::gen_matcher(m.0)).collect();
            all.push(m(R::gen_any(&any)));
        }
        if let Some(exactly) = &matcher.exactly {
            let mut list = vec![];
            for match_ in &exactly.list {
                list.push(self.map_temporary_match(rules, match_)?);
            }
            let list: Vec<_> = list.iter().map(|m| R::gen_matcher(m.0)).collect();
            all.push(m(R::gen_exactly(exactly.num, &list)))
        }
        Some(())
    }
}
