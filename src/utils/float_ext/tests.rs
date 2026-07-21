use crate::utils::float_ext::FloatExt;

#[test]
fn test_f32() {
    let max = f32::MAX_SAFE_INT;
    assert_eq!(max as f32 as u64, max);
    assert_eq!((max - 1) as f32 as u64, max - 1);
    assert_ne!((max + 1) as f32 as u64, max + 1);
}

#[test]
fn test_f64() {
    let max = f64::MAX_SAFE_INT;
    assert_eq!(max as f64 as u64, max);
    assert_eq!((max - 1) as f64 as u64, max - 1);
    assert_ne!((max + 1) as f64 as u64, max + 1);
}
