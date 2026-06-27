use crate::config::keycodes::keycode_from_name;

#[test]
fn test() {
    assert_eq!(keycode_from_name("esc"), Some(1));
    assert_eq!(keycode_from_name("numeric_star"), Some(0x20a));
    assert_eq!(keycode_from_name("macro15"), Some(0x29e));
    assert_eq!(keycode_from_name("aoeaoeuoaeu"), None);
}
