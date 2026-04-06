use {crate::utils::thread_id::ThreadId, std::thread};

#[test]
fn is_current() {
    let id = ThreadId::current();
    assert!(id.is_current());
    assert!(!id.is_not_current());
    let id = thread::spawn(move || {
        assert!(!id.is_current());
        assert!(id.is_not_current());
        let id = ThreadId::current();
        assert!(id.is_current());
        assert!(!id.is_not_current());
        id
    })
    .join()
    .unwrap();
    assert!(!id.is_current());
    assert!(id.is_not_current());
}
