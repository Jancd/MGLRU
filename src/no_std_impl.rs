const MAX_GENERATIONS: usize = 4;

struct Entry<K, V> {
    key: K,
    value: V,
    generation: usize,
    prev: usize,
    next: usize,
}

struct Generation {
    head: usize,
    tail: usize,
    len: usize,
}

const NONE: usize = usize::MAX;

impl Generation {
    const fn empty() -> Self {
        Self {
            head: NONE,
            tail: NONE,
            len: 0,
        }
    }
}

pub struct MglruCache<K, V, const CAP: usize> {
    entries: [Option<Entry<K, V>>; CAP],
    generations: [Generation; MAX_GENERATIONS],
    count: usize,
    hash_buckets_lo: [usize; CAP],
    hash_buckets_hi: [usize; CAP],
    free_list: [usize; CAP],
    free_len: usize,
    next_unused: usize,
}

impl<K: Clone + Eq + Hash, V, const CAP: usize> MglruCache<K, V, CAP> {
    pub fn new() -> Self {
        assert!(CAP > 0);
        Self {
            entries: [const { None }; CAP],
            generations: [
                Generation::empty(),
                Generation::empty(),
                Generation::empty(),
                Generation::empty(),
            ],
            count: 0,
            hash_buckets_lo: [NONE; CAP],
            hash_buckets_hi: [NONE; CAP],
            free_list: [NONE; CAP],
            free_len: 0,
            next_unused: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn capacity(&self) -> usize {
        CAP
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let idx = self.find_index(key)?;
        self.promote(idx);
        Some(&self.entries[idx].as_ref().unwrap().value)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let idx = self.find_index(key)?;
        self.promote(idx);
        Some(&mut self.entries[idx].as_mut().unwrap().value)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(idx) = self.find_index(&key) {
            let entry = self.entries[idx].as_mut().unwrap();
            let old = core::mem::replace(&mut entry.value, value);
            self.promote(idx);
            return Some(old);
        }

        if self.count >= CAP {
            self.evict();
        }

        let idx = self.alloc_slot();
        self.entries[idx] = Some(Entry {
            key: key.clone(),
            value,
            generation: 0,
            prev: NONE,
            next: NONE,
        });
        self.count += 1;
        self.hash_insert_idx(idx);
        self.push_front(0, idx);
        None
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let idx = self.find_index(key)?;
        let g = self.entries[idx].as_ref().unwrap().generation;
        self.unlink(g, idx);
        self.hash_remove_idx(idx);
        let entry = self.entries[idx].take().unwrap();
        self.count -= 1;
        self.free_slot(idx);
        Some(entry.value)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.find_index_const(key).is_some()
    }

    pub fn age(&mut self) {
        for g in (0..MAX_GENERATIONS - 1).rev() {
            if self.generations[g].head == NONE {
                continue;
            }
            let src_head = self.generations[g].head;
            let src_tail = self.generations[g].tail;
            let src_len = self.generations[g].len;

            let mut cursor = src_head;
            while cursor != NONE {
                let e = self.entries[cursor].as_mut().unwrap();
                e.generation = g + 1;
                cursor = e.next;
            }

            let dst_head = self.generations[g + 1].head;
            self.entries[src_tail].as_mut().unwrap().next = dst_head;
            if dst_head != NONE {
                self.entries[dst_head].as_mut().unwrap().prev = src_tail;
            }

            let dst_new_tail = if self.generations[g + 1].tail != NONE {
                self.generations[g + 1].tail
            } else {
                src_tail
            };
            let dst_new_len = self.generations[g + 1].len + src_len;

            self.generations[g + 1].head = src_head;
            self.generations[g + 1].tail = dst_new_tail;
            self.generations[g + 1].len = dst_new_len;

            self.generations[g] = Generation::empty();
        }
    }

    fn promote(&mut self, idx: usize) {
        let cur_gen = self.entries[idx].as_ref().unwrap().generation;
        if cur_gen == 0 {
            self.unlink(cur_gen, idx);
            self.push_front(0, idx);
            return;
        }
        let new_gen = cur_gen - 1;
        self.unlink(cur_gen, idx);
        self.entries[idx].as_mut().unwrap().generation = new_gen;
        self.push_front(new_gen, idx);
    }

    fn evict(&mut self) {
        for g in (0..MAX_GENERATIONS).rev() {
            if self.generations[g].tail != NONE {
                let tail = self.generations[g].tail;
                self.hash_remove_idx(tail);
                self.unlink(g, tail);
                self.entries[tail] = None;
                self.count -= 1;
                self.free_slot(tail);
                return;
            }
        }
    }

    fn alloc_slot(&mut self) -> usize {
        if self.free_len > 0 {
            self.free_len -= 1;
            return self.free_list[self.free_len];
        }
        debug_assert!(self.next_unused < CAP);
        let idx = self.next_unused;
        self.next_unused += 1;
        idx
    }

    fn free_slot(&mut self, idx: usize) {
        debug_assert!(self.free_len < CAP);
        self.free_list[self.free_len] = idx;
        self.free_len += 1;
    }

    fn push_front(&mut self, g: usize, idx: usize) {
        let old_head = self.generations[g].head;

        let e = self.entries[idx].as_mut().unwrap();
        e.prev = NONE;
        e.next = old_head;

        if old_head != NONE {
            self.entries[old_head].as_mut().unwrap().prev = idx;
        } else {
            self.generations[g].tail = idx;
        }
        self.generations[g].head = idx;
        self.generations[g].len += 1;
    }

    fn unlink(&mut self, g: usize, idx: usize) {
        let e = self.entries[idx].as_ref().unwrap();
        let prev = e.prev;
        let next = e.next;

        if prev != NONE {
            self.entries[prev].as_mut().unwrap().next = next;
        } else {
            self.generations[g].head = next;
        }

        if next != NONE {
            self.entries[next].as_mut().unwrap().prev = prev;
        } else {
            self.generations[g].tail = prev;
        }

        let e = self.entries[idx].as_mut().unwrap();
        e.prev = NONE;
        e.next = NONE;
        self.generations[g].len -= 1;
    }

    // --- Minimal hash map (open addressing, linear probing) ---

    fn hash_of(key: &K) -> usize {
        key.hash_value()
    }

    fn hash_capacity() -> usize {
        CAP * 2
    }

    fn hash_bucket_get(&self, bucket: usize) -> usize {
        if bucket < CAP {
            self.hash_buckets_lo[bucket]
        } else {
            self.hash_buckets_hi[bucket - CAP]
        }
    }

    fn hash_bucket_set(&mut self, bucket: usize, value: usize) {
        if bucket < CAP {
            self.hash_buckets_lo[bucket] = value;
        } else {
            self.hash_buckets_hi[bucket - CAP] = value;
        }
    }

    fn find_index(&self, key: &K) -> Option<usize> {
        self.find_index_const(key)
    }

    fn find_index_const(&self, key: &K) -> Option<usize> {
        if CAP == 0 {
            return None;
        }
        let hash_cap = Self::hash_capacity();
        let mut bucket = Self::hash_of(key) % hash_cap;
        for _ in 0..hash_cap {
            let slot = self.hash_bucket_get(bucket);
            if slot == NONE {
                return None;
            }
            if let Some(e) = &self.entries[slot] {
                if e.key == *key {
                    return Some(slot);
                }
            }
            bucket = (bucket + 1) % hash_cap;
        }
        None
    }

    fn hash_insert_idx(&mut self, idx: usize) {
        let hash = {
            let key = &self.entries[idx].as_ref().unwrap().key;
            Self::hash_of(key)
        };
        let hash_cap = Self::hash_capacity();
        let mut bucket = hash % hash_cap;
        for _ in 0..hash_cap {
            if self.hash_bucket_get(bucket) == NONE {
                self.hash_bucket_set(bucket, idx);
                return;
            }
            bucket = (bucket + 1) % hash_cap;
        }
        unreachable!()
    }

    fn hash_remove_idx(&mut self, idx: usize) {
        let hash = {
            let key = &self.entries[idx].as_ref().unwrap().key;
            Self::hash_of(key)
        };
        let hash_cap = Self::hash_capacity();
        let mut bucket = hash % hash_cap;
        for _ in 0..hash_cap {
            let slot = self.hash_bucket_get(bucket);
            if slot == NONE {
                return;
            }
            if slot == idx {
                self.hash_bucket_set(bucket, NONE);
                let mut next_bucket = (bucket + 1) % hash_cap;
                for _ in 0..hash_cap {
                    let ns = self.hash_bucket_get(next_bucket);
                    if ns == NONE {
                        break;
                    }
                    self.hash_bucket_set(next_bucket, NONE);
                    self.hash_insert_idx(ns);
                    next_bucket = (next_bucket + 1) % hash_cap;
                }
                return;
            }
            bucket = (bucket + 1) % hash_cap;
        }
    }
}

impl<K: Clone + Eq + Hash, V, const CAP: usize> Default for MglruCache<K, V, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Hash {
    fn hash_value(&self) -> usize;
}

impl Hash for i32 {
    fn hash_value(&self) -> usize {
        let mut x = *self as u64;
        x = x.wrapping_mul(0x9E3779B97F4A7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
        x ^= x >> 31;
        x as usize
    }
}

impl Hash for u64 {
    fn hash_value(&self) -> usize {
        let mut x = *self;
        x = x.wrapping_mul(0x9E3779B97F4A7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
        x ^= x >> 31;
        x as usize
    }
}

impl Hash for usize {
    fn hash_value(&self) -> usize {
        (*self as u64).hash_value()
    }
}

impl Hash for &str {
    fn hash_value(&self) -> usize {
        let mut h: usize = 5381;
        for b in self.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as usize);
        }
        h
    }
}
