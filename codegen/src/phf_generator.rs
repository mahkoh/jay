use {
    crate::{phf, phf::PhfHash},
    rand::{RngExt, SeedableRng, distr::StandardUniform, rngs::Xoshiro128PlusPlus},
};

const DEFAULT_LAMBDA: usize = 5;

pub struct HashState {
    pub key: u64,
    pub disps: Vec<(u32, u32)>,
    pub map: Vec<usize>,
}

pub fn generate_hash<T>(entries: &[T]) -> HashState
where
    T: PhfHash,
{
    Xoshiro128PlusPlus::from_seed([42; 16])
        .sample_iter(StandardUniform)
        .find_map(|key| try_generate_hash(entries, key))
        .expect("failed to solve PHF")
}

fn try_generate_hash<T>(entries: &[T], key: u64) -> Option<HashState>
where
    T: PhfHash,
{
    // This code is copied from the rust-phf project, there released under the MIT
    // license. Unlike rust-phf, this number of buckets created by this code is always
    // a power of two. This significantly improves performance of certain micro
    // benchmarks.

    struct Bucket {
        idx: usize,
        keys: Vec<usize>,
    }

    let hashes: Vec<_> = entries.iter().map(|entry| phf::hash(entry, key)).collect();

    let buckets_len = hashes.len().div_ceil(DEFAULT_LAMBDA);
    let buckets_len = buckets_len.next_power_of_two();
    let mut buckets = (0..buckets_len)
        .map(|i| Bucket {
            idx: i,
            keys: vec![],
        })
        .collect::<Vec<_>>();

    for (i, hash) in hashes.iter().enumerate() {
        buckets[(hash.g % (buckets_len as u32)) as usize]
            .keys
            .push(i);
    }

    // Sort descending
    buckets.sort_by(|a, b| a.keys.len().cmp(&b.keys.len()).reverse());

    let table_len = hashes.len();
    let mut map = vec![None; table_len];
    let mut disps = vec![(0u32, 0u32); buckets_len];

    // store whether an element from the bucket being placed is
    // located at a certain position, to allow for efficient overlap
    // checks. It works by storing the generation in each cell and
    // each new placement-attempt is a new generation, so you can tell
    // if this is legitimately full by checking that the generations
    // are equal. (A u64 is far too large to overflow in a reasonable
    // time for current hardware.)
    let mut try_map = vec![0u64; table_len];
    let mut generation = 0u64;

    // the actual values corresponding to the markers above, as
    // (index, key) pairs, for adding to the main map once we've
    // chosen the right disps.
    let mut values_to_add = vec![];

    'buckets: for bucket in &buckets {
        for d1 in 0..(table_len as u32) {
            'disps: for d2 in 0..(table_len as u32) {
                values_to_add.clear();
                generation += 1;

                for &key in &bucket.keys {
                    let idx = (phf::displace(hashes[key].f1, hashes[key].f2, d1, d2)
                        % (table_len as u32)) as usize;
                    if map[idx].is_some() || try_map[idx] == generation {
                        continue 'disps;
                    }
                    try_map[idx] = generation;
                    values_to_add.push((idx, key));
                }

                // We've picked a good set of disps
                disps[bucket.idx] = (d1, d2);
                for &(idx, key) in &values_to_add {
                    map[idx] = Some(key);
                }
                continue 'buckets;
            }
        }

        // Unable to find displacements for a bucket
        return None;
    }

    Some(HashState {
        key,
        disps,
        map: map.into_iter().map(|i| i.unwrap()).collect(),
    })
}
