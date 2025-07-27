use {
    crate::rect::{Rect, Region},
    jay_algorithms::rect::{NoTag, RectRaw},
};

#[test]
fn union1() {
    let r1 = Region::new(Rect::new(0, 0, 10, 10).unwrap());
    let r2_ = Region::new(Rect::new(5, 5, 15, 15).unwrap());
    let r2 = Region::new(Rect::new(10, 10, 20, 20).unwrap());
    let r3 = r1.union_cow(&r2);
    let r3 = r3.union_cow(&r2_);
    assert_eq!(r3.extents, Rect::new(0, 0, 20, 20).unwrap());
    assert_eq!(
        &r3.rects[..],
        &[
            Rect::new(0, 0, 10, 5).unwrap().raw,
            Rect::new(0, 5, 15, 10).unwrap().raw,
            Rect::new(5, 10, 20, 15).unwrap().raw,
            Rect::new(10, 15, 20, 20).unwrap().raw,
        ]
    );
}

#[test]
fn union2() {
    let r1 = Region::new(Rect::new(0, 0, 10, 10).unwrap());
    let r2 = Region::new(Rect::new(0, 10, 10, 20).unwrap());
    let r3 = r1.union_cow(&r2);
    assert_eq!(r3.extents, Rect::new(0, 0, 10, 20).unwrap());
    assert_eq!(&r3.rects[..], &[Rect::new(0, 0, 10, 20).unwrap().raw,]);
}

#[test]
fn subtract1() {
    let r1 = Region::new(Rect::new(0, 0, 20, 20).unwrap());
    let r2 = Region::new(Rect::new(5, 5, 15, 15).unwrap());
    let r3 = r1.subtract_cow(&r2);
    assert_eq!(r3.extents, Rect::new(0, 0, 20, 20).unwrap());
    assert_eq!(
        &r3.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 20,
                y2: 5,
                tag: NoTag,
            },
            RectRaw {
                x1: 0,
                y1: 5,
                x2: 5,
                y2: 15,
                tag: NoTag,
            },
            RectRaw {
                x1: 15,
                y1: 5,
                x2: 20,
                y2: 15,
                tag: NoTag,
            },
            RectRaw {
                x1: 0,
                y1: 15,
                x2: 20,
                y2: 20,
                tag: NoTag,
            },
        ]
    );
}

#[test]
fn rects_to_bands() {
    let rects = [
        Rect::new_unchecked_danger(0, 0, 10, 10),
        Rect::new_unchecked_danger(5, 0, 30, 10),
        Rect::new_unchecked_danger(30, 5, 50, 15),
    ];
    let r = Region::from_rects(&rects[..]);
    // println!("{:#?}", r.rects);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 30,
                y2: 5,
                tag: NoTag,
            },
            RectRaw {
                x1: 0,
                y1: 5,
                x2: 50,
                y2: 10,
                tag: NoTag,
            },
            RectRaw {
                x1: 30,
                y1: 10,
                x2: 50,
                y2: 15,
                tag: NoTag,
            },
        ]
    );
}

#[test]
fn rects_to_bands2() {
    let rects = [
        Rect::new_unchecked_danger(0, 0, 10, 10),
        Rect::new_unchecked_danger(0, 10, 10, 20),
    ];
    let r = Region::from_rects(&rects[..]);
    // println!("{:#?}", r.rects);
    assert_eq!(&r.rects[..], &[Rect::new(0, 0, 10, 20).unwrap().raw,]);
}

#[test]
fn rects_to_bands_tagged1() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 200, 200, 1),
        Rect::new_unchecked_danger_tagged(50, 50, 150, 150, 0),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 200,
                y2: 50,
                tag: 1,
            },
            RectRaw {
                x1: 0,
                y1: 50,
                x2: 50,
                y2: 150,
                tag: 1,
            },
            RectRaw {
                x1: 50,
                y1: 50,
                x2: 150,
                y2: 150,
                tag: 0,
            },
            RectRaw {
                x1: 150,
                y1: 50,
                x2: 200,
                y2: 150,
                tag: 1,
            },
            RectRaw {
                x1: 0,
                y1: 150,
                x2: 200,
                y2: 200,
                tag: 1,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged2() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 200, 200, 1),
        Rect::new_unchecked_danger_tagged(50, 50, 150, 150, 0),
        Rect::new_unchecked_danger_tagged(60, 60, 140, 140, 2),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 200,
                y2: 50,
                tag: 1,
            },
            RectRaw {
                x1: 0,
                y1: 50,
                x2: 50,
                y2: 150,
                tag: 1,
            },
            RectRaw {
                x1: 50,
                y1: 50,
                x2: 150,
                y2: 150,
                tag: 0,
            },
            RectRaw {
                x1: 150,
                y1: 50,
                x2: 200,
                y2: 150,
                tag: 1,
            },
            RectRaw {
                x1: 0,
                y1: 150,
                x2: 200,
                y2: 200,
                tag: 1,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged3() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 200, 200, 2),
        Rect::new_unchecked_danger_tagged(50, 50, 150, 150, 1),
        Rect::new_unchecked_danger_tagged(60, 60, 140, 140, 0),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 200,
                y2: 50,
                tag: 0,
            },
            RectRaw {
                x1: 0,
                y1: 50,
                x2: 50,
                y2: 60,
                tag: 0,
            },
            RectRaw {
                x1: 50,
                y1: 50,
                x2: 150,
                y2: 60,
                tag: 1,
            },
            RectRaw {
                x1: 150,
                y1: 50,
                x2: 200,
                y2: 60,
                tag: 0,
            },
            RectRaw {
                x1: 0,
                y1: 60,
                x2: 50,
                y2: 140,
                tag: 0,
            },
            RectRaw {
                x1: 50,
                y1: 60,
                x2: 60,
                y2: 140,
                tag: 1,
            },
            RectRaw {
                x1: 60,
                y1: 60,
                x2: 140,
                y2: 140,
                tag: 0,
            },
            RectRaw {
                x1: 140,
                y1: 60,
                x2: 150,
                y2: 140,
                tag: 1,
            },
            RectRaw {
                x1: 150,
                y1: 60,
                x2: 200,
                y2: 140,
                tag: 0,
            },
            RectRaw {
                x1: 0,
                y1: 140,
                x2: 50,
                y2: 150,
                tag: 0,
            },
            RectRaw {
                x1: 50,
                y1: 140,
                x2: 150,
                y2: 150,
                tag: 1,
            },
            RectRaw {
                x1: 150,
                y1: 140,
                x2: 200,
                y2: 150,
                tag: 0,
            },
            RectRaw {
                x1: 0,
                y1: 150,
                x2: 200,
                y2: 200,
                tag: 0,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged4() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 1),
        Rect::new_unchecked_danger_tagged(100, 0, 200, 200, 0),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 100,
                y2: 100,
                tag: 1,
            },
            RectRaw {
                x1: 100,
                y1: 0,
                x2: 200,
                y2: 100,
                tag: 0,
            },
            RectRaw {
                x1: 100,
                y1: 100,
                x2: 200,
                y2: 200,
                tag: 0,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged5() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 200, 100, 1),
        Rect::new_unchecked_danger_tagged(100, 0, 200, 100, 0),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 100,
                y2: 100,
                tag: 1,
            },
            RectRaw {
                x1: 100,
                y1: 0,
                x2: 200,
                y2: 100,
                tag: 0,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged6() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 200, 100, 1),
        Rect::new_unchecked_danger_tagged(100, 0, 300, 100, 0),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 100,
                y2: 100,
                tag: 1,
            },
            RectRaw {
                x1: 100,
                y1: 0,
                x2: 300,
                y2: 100,
                tag: 0,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged7() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 200, 100, 0),
        Rect::new_unchecked_danger_tagged(100, 0, 300, 200, 1),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 200,
                y2: 100,
                tag: 0,
            },
            RectRaw {
                x1: 200,
                y1: 0,
                x2: 300,
                y2: 100,
                tag: 1,
            },
            RectRaw {
                x1: 100,
                y1: 100,
                x2: 300,
                y2: 200,
                tag: 1,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged8() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 1),
        Rect::new_unchecked_danger_tagged(100, 0, 200, 100, 0),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 100,
                y2: 100,
                tag: 1,
            },
            RectRaw {
                x1: 100,
                y1: 0,
                x2: 200,
                y2: 100,
                tag: 0,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged9() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 1),
        Rect::new_unchecked_danger_tagged(100, 0, 200, 100, 1),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[RectRaw {
            x1: 0,
            y1: 0,
            x2: 200,
            y2: 100,
            tag: 1,
        },],
    );
}

#[test]
fn rects_to_bands_tagged10() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 1),
        Rect::new_unchecked_danger_tagged(0, 100, 100, 200, 1),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[RectRaw {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 200,
            tag: 1,
        },],
    );
}

#[test]
fn rects_to_bands_tagged11() {
    let rects = [Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 11)];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[RectRaw {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            tag: 1,
        },],
    );
}

#[test]
fn rects_to_bands_tagged12() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 11),
        Rect::new_unchecked_danger_tagged(200, 0, 300, 100, 10),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 100,
                y2: 100,
                tag: 1,
            },
            RectRaw {
                x1: 200,
                y1: 0,
                x2: 300,
                y2: 100,
                tag: 0,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged13() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 1),
        Rect::new_unchecked_danger_tagged(0, 100, 100, 200, 0),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[
            RectRaw {
                x1: 0,
                y1: 0,
                x2: 100,
                y2: 100,
                tag: 1,
            },
            RectRaw {
                x1: 0,
                y1: 100,
                x2: 100,
                y2: 200,
                tag: 0,
            },
        ],
    );
}

#[test]
fn rects_to_bands_tagged14() {
    let rects = [
        Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 1),
        Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 0),
    ];
    let r = Region::from_rects_tagged(&rects[..]);
    assert_eq!(
        &r.rects[..],
        &[RectRaw {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            tag: 0,
        },],
    );
}

#[test]
fn intersect1() {
    let rects = [Rect::new_unchecked_danger_tagged(0, 0, 100, 100, 0)];
    let r1 = Region::from_rects_tagged(&rects[..]);
    let rects = [Rect::new_unchecked_danger(100, 100, 200, 200)];
    let r2 = Region::from_rects2(&rects[..]);
    let r3 = r1.intersect_tagged(&r2);
    assert_eq!(&r3.rects[..], &[],);
}

#[test]
fn intersect2() {
    let rects = [Rect::new_unchecked_danger_tagged(0, 0, 200, 200, 0)];
    let r1 = Region::from_rects_tagged(&rects[..]);
    let rects = [Rect::new_unchecked_danger(50, 50, 150, 150)];
    let r2 = Region::from_rects2(&rects[..]);
    let r3 = r1.intersect_tagged(&r2);
    assert_eq!(
        &r3.rects[..],
        &[RectRaw {
            x1: 50,
            y1: 50,
            x2: 150,
            y2: 150,
            tag: 0,
        }],
    );
}

#[test]
fn intersect3() {
    macro_rules! t {
        ($l:expr, $r:expr, $t:expr) => {
            Rect::new_unchecked_danger_tagged($l, 0, $r, 1, $t)
        };
    }
    macro_rules! u {
        ($l:expr, $r:expr) => {
            Rect::new_unchecked_danger($l, 0, $r, 1)
        };
    }
    macro_rules! r {
        ($l:expr, $r:expr, $t:expr) => {
            RectRaw {
                x1: $l,
                y1: 0,
                x2: $r,
                y2: 1,
                tag: $t,
            }
        };
    }
    let rects = [
        t!(0, 100, 0),
        t!(110, 130, 1),
        t!(140, 160, 2),
        t!(170, 180, 0),
    ];
    let r1 = Region::from_rects_tagged(&rects[..]);
    let rects = [
        u!(10, 20),
        u!(50, 60),
        u!(70, 100),
        u!(120, 150),
        u!(170, 180),
    ];
    let r2 = Region::from_rects2(&rects[..]);
    let r3 = r1.intersect_tagged(&r2);
    assert_eq!(
        &r3.rects[..],
        &[
            r!(10, 20, 0),
            r!(50, 60, 0),
            r!(70, 100, 0),
            r!(120, 130, 1),
            r!(140, 150, 0),
            r!(170, 180, 0),
        ],
    );
}
