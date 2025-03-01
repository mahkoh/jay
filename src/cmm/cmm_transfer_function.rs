use linearize::Linearize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Linearize)]
pub enum TransferFunction {
    Srgb,
    Linear,
}
