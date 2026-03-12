use crate::std_impl::MglruCache as StdCache;
use crate::no_std_impl::MglruCache as NoStdCache;
use crate::no_std_impl::Hash as NoStdHash;

#[derive(Debug, PartialEq, Eq)]
struct NoStdConstHashKey(i32);

impl NoStdHash for NoStdConstHashKey {
    fn hash_value(&self) -> usize {
        14
    }
}

#[test]
fn std_basic_insert_and_get() {
    let mut cache = StdCache::new(4);
    cache.insert(1, "one");
    cache.insert(2, "two");
    cache.insert(3, "three");
    assert_eq!(cache.get(&1), Some(&"one"));
    assert_eq!(cache.get(&2), Some(&"two"));
    assert_eq!(cache.get(&3), Some(&"three"));
    assert_eq!(cache.len(), 3);
}

#[test]
fn std_eviction_removes_oldest_gen_tail() {
    let mut cache = StdCache::new(3);
    cache.insert(1, "a");
    cache.insert(2, "b");
    cache.insert(3, "c");
    // Age everything into older generations.
    cache.age();
    cache.age();
    cache.age();
    // Insert a 4th item; should evict 1 (oldest gen tail).
    cache.insert(4, "d");
    assert!(!cache.contains_key(&1));
    assert!(cache.contains_key(&2));
    assert!(cache.contains_key(&3));
    assert!(cache.contains_key(&4));
}

#[test]
fn std_access_promotes() {
    let mut cache = StdCache::new(3);
    cache.insert(1, "a");
    cache.insert(2, "b");
    cache.insert(3, "c");
    cache.age(); // all move to gen 1
    // Access key 1 → promotes back to gen 0
    cache.get(&1);
    cache.age(); // gen 0 → gen 1, gen 1 → gen 2
    cache.age(); // gen 1 → gen 2, gen 2 → gen 3
    // Now 2 and 3 are in gen 3, key 1 is in gen 2.
    // Evict should remove 2 or 3 (oldest gen tail), not 1.
    cache.insert(4, "d");
    assert!(cache.contains_key(&1));
    assert!(cache.contains_key(&4));
    assert_eq!(cache.len(), 3);
}

#[test]
fn std_update_existing_key() {
    let mut cache = StdCache::new(4);
    assert_eq!(cache.insert(1, "old"), None);
    assert_eq!(cache.insert(1, "new"), Some("old"));
    assert_eq!(cache.get(&1), Some(&"new"));
    assert_eq!(cache.len(), 1);
}

#[test]
fn std_remove() {
    let mut cache = StdCache::new(4);
    cache.insert(1, "a");
    cache.insert(2, "b");
    assert_eq!(cache.remove(&1), Some("a"));
    assert!(!cache.contains_key(&1));
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.remove(&1), None);
}

#[test]
fn std_age_shifts_generations() {
    let mut cache = StdCache::new(8);
    cache.insert(1, "a");
    cache.insert(2, "b");
    cache.age();
    cache.age();
    cache.age();
    // After 3 ages, both should be in gen 3 (max).
    // They should still be retrievable.
    assert_eq!(cache.get(&1), Some(&"a"));
    assert_eq!(cache.get(&2), Some(&"b"));
}

// ---- no_std impl tests ----

#[test]
fn nostd_basic_insert_and_get() {
    let mut cache = NoStdCache::<i32, &str, 4>::new();
    cache.insert(1, "one");
    cache.insert(2, "two");
    cache.insert(3, "three");
    assert_eq!(cache.get(&1), Some(&"one"));
    assert_eq!(cache.get(&2), Some(&"two"));
    assert_eq!(cache.get(&3), Some(&"three"));
    assert_eq!(cache.len(), 3);
}

#[test]
fn nostd_eviction_removes_oldest_gen_tail() {
    let mut cache = NoStdCache::<i32, &str, 3>::new();
    cache.insert(1, "a");
    cache.insert(2, "b");
    cache.insert(3, "c");
    cache.age();
    cache.age();
    cache.age();
    cache.insert(4, "d");
    assert!(!cache.contains_key(&1));
    assert!(cache.contains_key(&2));
    assert!(cache.contains_key(&3));
    assert!(cache.contains_key(&4));
}

#[test]
fn nostd_access_promotes() {
    let mut cache = NoStdCache::<i32, &str, 3>::new();
    cache.insert(1, "a");
    cache.insert(2, "b");
    cache.insert(3, "c");
    cache.age();
    cache.get(&1);
    cache.age();
    cache.age();
    cache.insert(4, "d");
    assert!(cache.contains_key(&1));
    assert!(cache.contains_key(&4));
    assert_eq!(cache.len(), 3);
}

#[test]
fn nostd_update_existing_key() {
    let mut cache = NoStdCache::<i32, &str, 4>::new();
    assert_eq!(cache.insert(1, "old"), None);
    assert_eq!(cache.insert(1, "new"), Some("old"));
    assert_eq!(cache.get(&1), Some(&"new"));
    assert_eq!(cache.len(), 1);
}

#[test]
fn nostd_remove() {
    let mut cache = NoStdCache::<i32, &str, 4>::new();
    cache.insert(1, "a");
    cache.insert(2, "b");
    assert_eq!(cache.remove(&1), Some("a"));
    assert!(!cache.contains_key(&1));
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.remove(&1), None);
}

#[test]
fn nostd_age_shifts_generations() {
    let mut cache = NoStdCache::<i32, &str, 8>::new();
    cache.insert(1, "a");
    cache.insert(2, "b");
    cache.age();
    cache.age();
    cache.age();
    assert_eq!(cache.get(&1), Some(&"a"));
    assert_eq!(cache.get(&2), Some(&"b"));
}

#[test]
fn std_heavy_churn() {
    let mut cache = StdCache::new(16);
    for i in 0..1000 {
        cache.insert(i, i * 10);
        if i % 7 == 0 {
            cache.age();
        }
    }
    assert_eq!(cache.len(), 16);
    // The most recently inserted keys should be present.
    for i in 984..1000 {
        assert!(cache.contains_key(&i));
    }
}

#[test]
fn nostd_heavy_churn() {
    let mut cache = NoStdCache::<i32, i32, 16>::new();
    for i in 0..1000 {
        cache.insert(i, i * 10);
        if i % 7 == 0 {
            cache.age();
        }
    }
    assert_eq!(cache.len(), 16);
    for i in 984..1000 {
        assert!(cache.contains_key(&i));
    }
}

#[test]
fn nostd_remove_insert_reuse_slots() {
    let mut cache = NoStdCache::<i32, i32, 32>::new();

    for i in 0..32 {
        cache.insert(i, i);
    }
    for i in 0..16 {
        assert_eq!(cache.remove(&i), Some(i));
    }
    for i in 100..116 {
        cache.insert(i, i * 2);
    }

    assert_eq!(cache.len(), 32);
    for i in 0..16 {
        assert!(!cache.contains_key(&i));
    }
    for i in 16..32 {
        assert!(cache.contains_key(&i));
    }
    for i in 100..116 {
        assert_eq!(cache.get(&i), Some(&(i * 2)));
    }
}

#[test]
#[should_panic]
fn nostd_zero_capacity_panics() {
    let _ = NoStdCache::<i32, i32, 0>::new();
}

#[test]
fn nostd_capacity_one_edge_behavior() {
    let mut cache = NoStdCache::<i32, i32, 1>::new();

    assert_eq!(cache.capacity(), 1);
    assert!(cache.is_empty());

    assert_eq!(cache.insert(1, 10), None);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get(&1), Some(&10));

    assert_eq!(cache.insert(1, 11), Some(10));
    assert_eq!(cache.get(&1), Some(&11));

    cache.age();
    cache.age();
    assert_eq!(cache.get(&1), Some(&11));

    assert_eq!(cache.insert(2, 20), None);
    assert_eq!(cache.len(), 1);
    assert!(!cache.contains_key(&1));
    assert_eq!(cache.get(&2), Some(&20));

    assert_eq!(cache.remove(&2), Some(20));
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
    assert_eq!(cache.remove(&2), None);

    assert_eq!(cache.insert(3, 30), None);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get(&3), Some(&30));
}

#[test]
fn nostd_remove_cluster_wraparound_integrity() {
    let mut cache = NoStdCache::<NoStdConstHashKey, i32, 8>::new();

    for i in 0..8 {
        cache.insert(NoStdConstHashKey(i), i * 10);
    }

    assert_eq!(cache.remove(&NoStdConstHashKey(3)), Some(30));
    assert_eq!(cache.remove(&NoStdConstHashKey(6)), Some(60));

    for i in [0, 1, 2, 4, 5, 7] {
        assert_eq!(cache.get(&NoStdConstHashKey(i)), Some(&(i * 10)));
    }

    assert_eq!(cache.insert(NoStdConstHashKey(100), 1000), None);
    assert_eq!(cache.insert(NoStdConstHashKey(101), 1010), None);
    assert_eq!(cache.len(), 8);
    assert_eq!(cache.get(&NoStdConstHashKey(100)), Some(&1000));
    assert_eq!(cache.get(&NoStdConstHashKey(101)), Some(&1010));
}
