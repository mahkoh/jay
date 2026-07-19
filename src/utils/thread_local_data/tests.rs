use crate::utils::rc_eq::rc_eq;
use crate::utils::thread_id::ThreadId;
use crate::utils::thread_local_data::ThreadLocalData;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::thread;

#[test]
fn test() {
    static RC: AtomicUsize = AtomicUsize::new(0);
    struct T;
    impl Drop for T {
        fn drop(&mut self) {
            RC.fetch_sub(1, Relaxed);
        }
    }

    assert_eq!(RC.load(Relaxed), 0);

    thread::spawn(|| {
        let data = ThreadLocalData::new(ThreadId::current());
        let get = || {
            data.get_or_create(|| {
                RC.fetch_add(1, Relaxed);
                Rc::new(T)
            })
        };
        assert_eq!(RC.load(Relaxed), 0);
        let t = get();
        assert_eq!(RC.load(Relaxed), 1);
        let t2 = get();
        assert_eq!(RC.load(Relaxed), 1);
        assert!(rc_eq(&t, &t2));
    })
    .join()
    .unwrap();

    assert_eq!(RC.load(Relaxed), 0);
}

#[test]
#[should_panic]
fn wrong_thread() {
    let data = thread::spawn(|| ThreadLocalData::new(ThreadId::current()))
        .join()
        .unwrap();
    data.get_or_create(|| Rc::new(()));
}
