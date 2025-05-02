use {
    crate::criteria::{
        CritLiteralOrRegex, RootMatcherMap,
        crit_graph::{CritRootCriterion, CritTarget},
    },
    std::marker::PhantomData,
};

pub struct CritMatchString<Target, A> {
    string: CritLiteralOrRegex,
    _phantom: PhantomData<(fn(&Target), A)>,
}

pub trait StringAccess<Target>: Sized + 'static
where
    Target: CritTarget,
{
    fn with_string(data: &Target, f: impl FnOnce(&str) -> bool) -> bool;
    fn nodes(
        roots: &Target::RootMatchers,
    ) -> &RootMatcherMap<Target, CritMatchString<Target, Self>>;
}

impl<Target, A> CritMatchString<Target, A> {
    #[expect(dead_code)]
    pub fn new(string: CritLiteralOrRegex) -> Self {
        Self {
            string,
            _phantom: Default::default(),
        }
    }
}

impl<Target, A> CritRootCriterion<Target> for CritMatchString<Target, A>
where
    Target: CritTarget,
    A: StringAccess<Target>,
{
    fn matches(&self, data: &Target) -> bool {
        A::with_string(data, |s| self.string.matches(s))
    }

    fn nodes(roots: &Target::RootMatchers) -> Option<&RootMatcherMap<Target, Self>> {
        Some(A::nodes(roots))
    }
}
