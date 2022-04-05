use bincode::{Decode, Encode};

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Connector(pub u64);
