use linearize::Linearize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Linearize)]
pub enum TransferFunction {
    Linear,
    St2084Pq,
    Bt1886,
    Gamma22,
    Gamma28,
    St240,
    Log100,
    Log316,
    St428,
}
