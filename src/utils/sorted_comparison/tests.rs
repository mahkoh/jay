use crate::utils::sorted_comparison::SortedResult;
use crate::utils::sorted_comparison::sorted_comparison;

#[test]
fn test() {
    let a = [2, 3, 4, 5, 7, 10];
    let b = [0, 3, 4, 6, 7, 8];
    let res: Vec<_> = sorted_comparison(&a, &b).collect();
    assert_eq!(
        res,
        [
            SortedResult::Right(&0),
            SortedResult::Left(&2),
            SortedResult::Equal(&3, &3),
            SortedResult::Equal(&4, &4),
            SortedResult::Left(&5),
            SortedResult::Right(&6),
            SortedResult::Equal(&7, &7),
            SortedResult::Right(&8),
            SortedResult::Left(&10),
        ]
    );
}
