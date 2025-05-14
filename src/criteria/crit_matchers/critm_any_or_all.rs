use {
    crate::criteria::crit_graph::{CritMiddleCriterion, CritTarget, CritUpstreamNode},
    std::{marker::PhantomData, rc::Rc},
};

pub struct CritMatchAnyOrAll<Target>
where
    Target: CritTarget,
{
    all: bool,
    total: usize,
    _phantom: PhantomData<fn(&Target)>,
}

impl<Target> CritMatchAnyOrAll<Target>
where
    Target: CritTarget,
{
    pub fn new(upstream: &[Rc<dyn CritUpstreamNode<Target>>], all: bool) -> Self {
        Self {
            all,
            total: upstream.len(),
            _phantom: Default::default(),
        }
    }
}

impl<Target> CritMiddleCriterion<Target> for CritMatchAnyOrAll<Target>
where
    Target: CritTarget,
{
    type Data = usize;
    type Not = Self;

    fn update_matched(&self, _data: &Target, node: &mut usize, matched: bool) -> bool {
        if matched {
            *node += 1;
        } else {
            *node -= 1;
        }
        if self.all {
            *node == self.total
        } else {
            *node > 0
        }
    }

    fn pull(&self, upstream: &[Rc<dyn CritUpstreamNode<Target>>], node: &Target) -> bool {
        for upstream in upstream {
            if upstream.pull(node) {
                if !self.all {
                    return true;
                }
            } else {
                if self.all {
                    return false;
                }
            }
        }
        self.all
    }

    fn not(&self) -> Self
    where
        Self: Sized,
    {
        Self {
            all: !self.all,
            total: self.total,
            _phantom: Default::default(),
        }
    }
}
