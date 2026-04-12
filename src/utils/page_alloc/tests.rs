use crate::utils::page_alloc::{PAGE_ALLOC_PAGE_SIZE, PageAllocCtx};

#[test]
fn test() {
    let ctx = PageAllocCtx::default();
    let alloc = ctx.create_alloc(1);
    assert_eq!(alloc.allocate(1).unwrap().offset(), 0);
    assert_eq!(alloc.allocate(1).unwrap().offset(), 0);
    let entry1 = alloc.allocate(1).unwrap();
    assert_eq!(entry1.offset(), 0);
    assert_eq!(alloc.allocate(1).unwrap().offset(), 1);
    let entry2 = alloc.allocate(1).unwrap();
    assert_eq!(entry2.offset(), 1);
    assert_eq!(alloc.allocate(1).unwrap().offset(), 2);
    drop(entry1);
    assert_eq!(alloc.allocate(1).unwrap().offset(), 0);
    assert!(alloc.allocate(2).is_none());
    drop(entry2);
    assert_eq!(alloc.allocate(2).unwrap().offset(), 0);
    let entry3 = alloc.allocate(4096).unwrap();
    assert_eq!(entry3.offset(), 0);
    assert!(alloc.allocate(4096).is_none());
}

#[test]
fn is_in_alloc() {
    let ctx = PageAllocCtx::default();
    let alloc1 = ctx.create_alloc(1);
    let alloc2 = ctx.create_alloc(1);
    let entry1 = alloc1.allocate(1).unwrap();
    assert!(entry1.is_in_alloc(&alloc1));
    assert!(entry1.is_not_in_alloc(&alloc2));
}

#[test]
fn size() {
    let ctx = PageAllocCtx::default();
    let alloc1 = ctx.create_alloc(1);
    let alloc2 = ctx.create_alloc(2);
    assert_eq!(alloc1.size(), 1 * PAGE_ALLOC_PAGE_SIZE as usize);
    assert_eq!(alloc2.size(), 2 * PAGE_ALLOC_PAGE_SIZE as usize);
}

#[test]
fn non_power_of_two_alloc() {
    let ctx = PageAllocCtx::default();
    let alloc1 = ctx.create_alloc(1);
    let _entry1 = alloc1.allocate(PAGE_ALLOC_PAGE_SIZE as usize - 1).unwrap();
    assert!(alloc1.allocate(PAGE_ALLOC_PAGE_SIZE as usize - 1).is_none());
}

#[test]
fn offset() {
    let ctx = PageAllocCtx::default();
    let alloc1 = ctx.create_alloc(2);
    let entry1 = alloc1.allocate(PAGE_ALLOC_PAGE_SIZE as usize / 2).unwrap();
    let entry2 = alloc1.allocate(PAGE_ALLOC_PAGE_SIZE as usize / 2).unwrap();
    let _entry3 = alloc1.allocate(PAGE_ALLOC_PAGE_SIZE as usize / 2).unwrap();
    drop(entry1);
    drop(entry2);
    let entry4 = alloc1.allocate(PAGE_ALLOC_PAGE_SIZE as usize / 2).unwrap();
    assert_eq!(entry4.offset(), PAGE_ALLOC_PAGE_SIZE * 3 / 2);
}

#[test]
fn page_priority() {
    let ctx = PageAllocCtx::default();
    let alloc1 = ctx.create_alloc(2);
    let alloc = || alloc1.allocate(PAGE_ALLOC_PAGE_SIZE as usize / 4).unwrap();
    let entry1 = alloc();
    let entry2 = alloc();
    let entry3 = alloc();
    let entry4 = alloc();
    let entry5 = alloc();
    let entry6 = alloc();
    let entry7 = alloc();
    let entry8 = alloc();
    for (idx, entry) in [
        &entry1, &entry2, &entry3, &entry4, &entry5, &entry6, &entry7, &entry8,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(entry.offset(), idx as u32 * PAGE_ALLOC_PAGE_SIZE / 4);
    }
    drop(entry1);
    drop(entry2);
    drop(entry3);
    drop(entry5);
    let entry5 = alloc();
    let entry1 = alloc();
    let entry2 = alloc();
    let entry3 = alloc();
    for (idx, entry) in [
        &entry1, &entry2, &entry3, &entry4, &entry5, &entry6, &entry7, &entry8,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(entry.offset(), idx as u32 * PAGE_ALLOC_PAGE_SIZE / 4);
    }
    drop(entry1);
    drop(entry5);
    drop(entry6);
    drop(entry7);
    let entry1 = alloc();
    let entry5 = alloc();
    let entry6 = alloc();
    let entry7 = alloc();
    for (idx, entry) in [
        &entry1, &entry2, &entry3, &entry4, &entry5, &entry6, &entry7, &entry8,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(entry.offset(), idx as u32 * PAGE_ALLOC_PAGE_SIZE / 4);
    }
    drop(entry1);
    drop(entry2);
    drop(entry5);
    drop(entry6);
    let entry5 = alloc();
    let entry6 = alloc();
    let entry1 = alloc();
    let entry2 = alloc();
    for (idx, entry) in [
        &entry1, &entry2, &entry3, &entry4, &entry5, &entry6, &entry7, &entry8,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(entry.offset(), idx as u32 * PAGE_ALLOC_PAGE_SIZE / 4);
    }
}

#[test]
fn page_reuse() {
    let ctx = PageAllocCtx::default();
    let alloc1 = ctx.create_alloc(1);
    let entry1 = alloc1.allocate(1).unwrap();
    assert!(alloc1.allocate(2).is_none());
    drop(entry1);
    assert!(alloc1.allocate(2).is_some());
}

#[test]
fn page_reuse2() {
    let ctx = PageAllocCtx::default();
    let alloc1 = ctx.create_alloc(1);
    let entry1 = alloc1.allocate(2).unwrap();
    assert_eq!(entry1.offset(), 0);
    let entry2 = alloc1.allocate(2).unwrap();
    assert_eq!(entry2.offset(), 2);
    let entry3 = alloc1.allocate(2).unwrap();
    assert_eq!(entry3.offset(), 4);
    drop(entry1);
    drop(entry2);
    drop(entry3);
    let entry1 = alloc1.allocate(1).unwrap();
    assert_eq!(entry1.offset(), 0);
    let entry2 = alloc1.allocate(1).unwrap();
    assert_eq!(entry2.offset(), 1);
    let entry3 = alloc1.allocate(1).unwrap();
    assert_eq!(entry3.offset(), 2);
}
