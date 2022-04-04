use crate::rect::{Rect, Region};

#[test]
fn union1() {
    let r1 = Region::new(Rect::new(0, 0, 10, 10).unwrap());
    let r2_ = Region::new(Rect::new(5, 5, 15, 15).unwrap());
    let r2 = Region::new(Rect::new(10, 10, 20, 20).unwrap());
    let r3 = r1.union(&r2).union(&r2_);
    assert_eq!(r3.extents, Rect::new(0, 0, 20, 20).unwrap());
    assert_eq!(
        &r3.rects[..],
        &[
            Rect::new(0, 0, 10, 5).unwrap(),
            Rect::new(0, 5, 15, 10).unwrap(),
            Rect::new(5, 10, 20, 15).unwrap(),
            Rect::new(10, 15, 20, 20).unwrap(),
        ]
    );
}

#[test]
fn union2() {
    let r1 = Region::new(Rect::new(0, 0, 10, 10).unwrap());
    let r2 = Region::new(Rect::new(0, 10, 10, 20).unwrap());
    let r3 = r1.union(&r2);
    assert_eq!(r3.extents, Rect::new(0, 0, 10, 20).unwrap());
    assert_eq!(&r3.rects[..], &[Rect::new(0, 0, 10, 20).unwrap(),]);
}

#[test]
fn subtract1() {
    let r1 = Region::new(Rect::new(0, 0, 20, 20).unwrap());
    let r2 = Region::new(Rect::new(5, 5, 15, 15).unwrap());
    let r3 = r1.subtract(&r2);
    assert_eq!(r3.extents, Rect::new(0, 0, 20, 20).unwrap());
    assert_eq!(&r3.rects[..], &[Rect::new(0, 0, 10, 20).unwrap(),]);
}

#[test]
fn rects_to_bands() {
    let rects = [
        Rect::new_unchecked(0, 0, 10, 10),
        Rect::new_unchecked(5, 0, 30, 10),
        Rect::new_unchecked(30, 5, 50, 15),
    ];
    let r = Region::from_rects(&rects[..]);
    println!("{:#?}", r.rects);
    assert_eq!(&r.rects[..], &[Rect::new(0, 0, 10, 20).unwrap(),]);
}

#[test]
fn rects_to_bands2() {
    let rects = [
        Rect::new_unchecked(0, 0, 10, 10),
        Rect::new_unchecked(0, 10, 10, 20),
    ];
    let r = Region::from_rects(&rects[..]);
    println!("{:#?}", r.rects);
    assert_eq!(&r.rects[..], &[Rect::new(0, 0, 10, 20).unwrap(),]);
}
