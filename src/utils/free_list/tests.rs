use crate::utils::free_list::FreeList;

#[test]
fn test() {
    let list = FreeList::<u32, 3>::default();
    for i in 0..4097 {
        assert_eq!(list.acquire(), i);
    }
    list.release(100);
    assert_eq!(list.acquire(), 100);
    assert_eq!(list.acquire(), 4097);
    for i in 1..22 {
        list.release(i);
    }
    for i in 1..22 {
        assert_eq!(list.acquire(), i);
    }
    assert_eq!(list.acquire(), 4098);
    for i in 64..128 {
        list.release(i);
    }
    for i in 64..128 {
        assert_eq!(list.acquire(), i);
    }
    assert_eq!(list.acquire(), 4099);
    for i in 0..4100 {
        list.release(i);
    }
    for i in 0..4101 {
        assert_eq!(list.acquire(), i);
    }
}

#[test]
#[should_panic]
fn release_out_of_bounds() {
    let list = FreeList::<u32, 3>::default();
    list.acquire();
    list.release(500);
}
