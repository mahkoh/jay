use {
    crate::utils::opaque::{Opaque, opaque},
    std::str::FromStr,
};

#[test]
fn roundtrip() {
    let v1 = opaque();
    let s = v1.to_string();
    let v2 = Opaque::from_str(&s).unwrap();
    assert_eq!(v1, v2);
}
