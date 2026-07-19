use crate::criteria::crit_graph::CritRootCriterion;
use crate::criteria::tlm::RootMatchers;
use crate::criteria::tlm::TlmRootMatcherMap;
use crate::ifs::wl_seat::SeatId;
use crate::tree::ToplevelData;

pub struct TlmMatchSeatFocus {
    id: SeatId,
}

impl TlmMatchSeatFocus {
    pub fn new(id: SeatId) -> TlmMatchSeatFocus {
        Self { id }
    }
}

impl CritRootCriterion<ToplevelData> for TlmMatchSeatFocus {
    fn matches(&self, data: &ToplevelData) -> bool {
        data.seat_foci.contains(&self.id)
    }

    fn nodes(roots: &RootMatchers) -> Option<&TlmRootMatcherMap<Self>> {
        Some(&roots.seat_foci)
    }
}
