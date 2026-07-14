use crate::utils::{ptr_ext::PtrExt, random_cache::RandomCache};

#[test]
fn insert() {
    let cache = RandomCache::new(16);

    cache.insert(1, "a");
    assert_eq!(cache.get(&1).unwrap().value, "a");
}

#[test]
fn last_retained() {
    let cache = RandomCache::new(16);

    for i in 0..64 {
        cache.insert(i, -i);
    }

    assert_eq!(cache.get(&63).unwrap().value, -63);
}

#[test]
fn last_stored_retained() {
    let cache = RandomCache::new(16);

    let mut cached = vec![];

    for i in 0..64 {
        cached.push(cache.insert(i, -i));
    }

    for i in 0..64 {
        let cached = cache.get(&i).unwrap();
        assert_eq!(*cached.key(), i);
        assert_eq!(**cached, -i);
    }

    unsafe {
        assert_eq!(cache.inner.mut_.get().deref().map.len(), 64);
    }

    drop(cached);

    let mut present = 0;
    for i in 0..64 {
        present += cache.get(&i).is_some() as u32;
    }

    assert_eq!(present, 16);
    unsafe {
        assert_eq!(cache.inner.mut_.get().deref().map.len(), 16);
    }
}

#[test]
fn some_present() {
    loop {
        let cache = RandomCache::new(16);

        for i in 0..1024 {
            cache.insert(i, -i);
        }

        let mut present = 0;
        for i in (1024 - 16)..1024 {
            present += cache.get(&i).is_some() as u32;
        }

        // The expected occupancy is ~80% * 16 > 12.
        if present >= 10 {
            return;
        }
    }
}

#[test]
fn mark_used() {
    let cache = RandomCache::new(16);

    for i in 0..1024 {
        cache.insert(i, -i);

        let first = cache.get(&0).unwrap();
        first.mark_used();
    }
}

#[cfg(not(debug_assertions))]
#[test]
fn perf() {
    use std::time::Instant;

    const SIZE: usize = 128;
    let cache = RandomCache::new(SIZE);

    const COUNT: u32 = 1_000_000;
    for i in 0..COUNT {
        cache.insert(i, i);
    }

    let start = Instant::now();
    for i in 0..COUNT {
        cache.insert(i, i);
    }
    let cost = start.elapsed() / COUNT;
    println!("insert: {cost:?}");

    let start = Instant::now();
    for i in 0..COUNT {
        cache.get(&i);
    }
    let cost = start.elapsed() / COUNT;
    println!("get: {cost:?}");

    let mut occupancy = 0;
    for i in (COUNT - 128)..COUNT {
        occupancy += cache.get(&i).is_some() as u32;
    }
    println!("occupancy: {}", occupancy as f64 / SIZE as f64);
}
