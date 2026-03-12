use std::collections::HashMap;

const MAX_GENERATIONS: usize = 4;

struct Entry<K, V> {
    key: K,
    value: V,
    generation: usize,
    prev: Option<usize>,
    next: Option<usize>,
}

struct Generation {
    head: Option<usize>,
    tail: Option<usize>,
    len: usize,
}

impl Generation {
    fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }
}

pub struct MglruCache<K, V> {
    entries: Vec<Option<Entry<K, V>>>,
    free_list: Vec<usize>,
    map: HashMap<K, usize>,
    generations: [Generation; MAX_GENERATIONS],
    capacity: usize,
}

impl<K: Clone + Eq + std::hash::Hash, V> MglruCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            entries: Vec::with_capacity(capacity),
            free_list: Vec::with_capacity(capacity),
            map: HashMap::with_capacity(capacity),
            generations: [
                Generation::new(),
                Generation::new(),
                Generation::new(),
                Generation::new(),
            ],
            capacity,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let idx = *self.map.get(key)?;
        self.promote(idx);
        Some(&self.entries[idx].as_ref().unwrap().value)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let idx = *self.map.get(key)?;
        self.promote(idx);
        Some(&mut self.entries[idx].as_mut().unwrap().value)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(&idx) = self.map.get(&key) {
            let entry = self.entries[idx].as_mut().unwrap();
            let old = std::mem::replace(&mut entry.value, value);
            self.promote(idx);
            return Some(old);
        }

        if self.map.len() >= self.capacity {
            self.evict();
        }

        let idx = self.alloc(Entry {
            key: key.clone(),
            value,
            generation: 0,
            prev: None,
            next: None,
        });

        self.push_front(0, idx);
        self.map.insert(key, idx);
        None
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let idx = self.map.remove(key)?;
        let g = self.entries[idx].as_ref().unwrap().generation;
        self.unlink(g, idx);
        let entry = self.entries[idx].take().unwrap();
        self.free_list.push(idx);
        Some(entry.value)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
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
            if let Some(tail) = self.generations[g].tail {
                self.map.remove(&self.entries[tail].as_ref().unwrap().key);
                self.unlink(g, tail);
                self.entries[tail] = None;
                self.free_list.push(tail);
                return;
            }
        }
    }

    pub fn age(&mut self) {
        for g in (0..MAX_GENERATIONS - 1).rev() {
            let src = &self.generations[g];
            if src.head.is_none() {
                continue;
            }
            let src_head = src.head.unwrap();
            let src_tail = src.tail.unwrap();
            let src_len = src.len;

            let mut cursor = Some(src_head);
            while let Some(c) = cursor {
                let e = self.entries[c].as_mut().unwrap();
                e.generation = g + 1;
                cursor = e.next;
            }

            let dst_head = self.generations[g + 1].head;

            self.entries[src_tail].as_mut().unwrap().next = dst_head;
            if let Some(dh) = dst_head {
                self.entries[dh].as_mut().unwrap().prev = Some(src_tail);
            }

            let dst_new_tail = if self.generations[g + 1].tail.is_some() {
                self.generations[g + 1].tail
            } else {
                Some(src_tail)
            };
            let dst_new_len = self.generations[g + 1].len + src_len;

            self.generations[g + 1].head = Some(src_head);
            self.generations[g + 1].tail = dst_new_tail;
            self.generations[g + 1].len = dst_new_len;

            self.generations[g] = Generation::new();
        }
    }

    fn alloc(&mut self, entry: Entry<K, V>) -> usize {
        if let Some(idx) = self.free_list.pop() {
            self.entries[idx] = Some(entry);
            idx
        } else {
            let idx = self.entries.len();
            self.entries.push(Some(entry));
            idx
        }
    }

    fn push_front(&mut self, g: usize, idx: usize) {
        let old_head = self.generations[g].head;
        let e = self.entries[idx].as_mut().unwrap();
        e.prev = None;
        e.next = old_head;

        if let Some(oh) = old_head {
            self.entries[oh].as_mut().unwrap().prev = Some(idx);
        } else {
            self.generations[g].tail = Some(idx);
        }
        self.generations[g].head = Some(idx);
        self.generations[g].len += 1;
    }

    fn unlink(&mut self, g: usize, idx: usize) {
        let e = self.entries[idx].as_ref().unwrap();
        let prev = e.prev;
        let next = e.next;

        if let Some(p) = prev {
            self.entries[p].as_mut().unwrap().next = next;
        } else {
            self.generations[g].head = next;
        }

        if let Some(n) = next {
            self.entries[n].as_mut().unwrap().prev = prev;
        } else {
            self.generations[g].tail = prev;
        }

        let e = self.entries[idx].as_mut().unwrap();
        e.prev = None;
        e.next = None;
        self.generations[g].len -= 1;
    }
}
