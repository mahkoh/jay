use crate::utils::spaces::spaces;

#[test]
fn test() {
    assert_eq!(spaces(0).to_string(), "");
    assert_eq!(spaces(1).to_string(), " ");
    assert_eq!(spaces(2).to_string(), "  ");
    assert_eq!(spaces(2).to_string(), "  ");
    assert_eq!(spaces(3).to_string(), "   ");
    assert_eq!(spaces(2).to_string(), "  ");
    assert_eq!(spaces(1).to_string(), " ");
    assert_eq!(spaces(0).to_string(), "");
}
