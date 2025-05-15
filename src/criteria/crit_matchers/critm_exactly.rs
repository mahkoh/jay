use {
    crate::criteria::crit_graph::{CritMiddleCriterion, CritTarget, CritUpstreamNode},
    std::{marker::PhantomData, rc::Rc},
};

pub struct CritMatchExactly<Target> {
    total: usize,
    num: usize,
    not: bool,
    _phantom: PhantomData<fn(&Target)>,
}

impl<Target> CritMatchExactly<Target> {
    pub fn new(upstream: &[Rc<dyn CritUpstreamNode<Target>>], num: usize) -> Self {
        Self {
            total: upstream.len(),
            num,
            not: false,
            _phantom: Default::default(),
        }
    }
}

impl<Target> CritMiddleCriterion<Target> for CritMatchExactly<Target>
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
        (*node == self.num) ^ self.not
    }

    fn pull(&self, upstream: &[Rc<dyn CritUpstreamNode<Target>>], node: &Target) -> bool {
        let mut n = 0;
        for upstream in upstream {
            if upstream.pull(node) {
                n += 1;
            }
        }
        (n == self.num) ^ self.not
    }

    fn not(&self) -> Self
    where
        Self: Sized,
    {
        Self {
            total: self.total,
            num: self.total - self.num,
            not: !self.not,
            _phantom: Default::default(),
        }
    }
}
