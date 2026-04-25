use std::collections::{HashMap, VecDeque};

use crate::line::Line;

/// LRU cache of parsed Line objects, keyed by line index.
pub struct LineCache {
    entries: HashMap<usize, Line>,
    /// Access order: front = oldest, back = most recently used.
    order: VecDeque<usize>,
    capacity: usize,
}

impl LineCache {
    /// Create a new cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Get a cached line by index, marking it as most recently used.
    pub fn get(&mut self, index: usize) -> Option<&Line> {
        if self.entries.contains_key(&index) {
            self.touch(index);
            self.entries.get(&index)
        } else {
            None
        }
    }

    /// Insert a line into the cache, evicting the oldest entry if at capacity.
    /// Returns a reference to the inserted line.
    pub fn insert(&mut self, index: usize, line: Line) -> &Line {
        if self.entries.contains_key(&index) {
            // Already cached — update value and move to back of order.
            self.entries.insert(index, line);
            self.touch(index);
        } else {
            if self.entries.len() >= self.capacity {
                self.evict_oldest();
            }
            self.entries.insert(index, line);
            self.order.push_back(index);
        }
        self.entries.get(&index).unwrap()
    }

    /// Move an index to the back (most recently used).
    fn touch(&mut self, index: usize) {
        if let Some(pos) = self.order.iter().position(|&i| i == index) {
            self.order.remove(pos);
        }
        self.order.push_back(index);
    }

    /// Evict the oldest (front) entry.
    fn evict_oldest(&mut self) {
        if let Some(oldest) = self.order.pop_front() {
            self.entries.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(text: &str) -> Line {
        Line::new(0, text.to_string())
    }

    #[test]
    fn insert_and_get() {
        let mut cache = LineCache::new(10);
        cache.insert(0, make_line("hello"));
        cache.insert(5, make_line("world"));
        assert_eq!(cache.get(0).unwrap().raw(), "hello");
        assert_eq!(cache.get(5).unwrap().raw(), "world");
        assert!(cache.get(1).is_none());
    }

    #[test]
    fn eviction_at_capacity() {
        let mut cache = LineCache::new(2);
        cache.insert(0, make_line("a"));
        cache.insert(1, make_line("b"));
        // Cache is full. Inserting a third evicts the oldest (index 0).
        cache.insert(2, make_line("c"));
        assert!(cache.get(0).is_none());
        assert_eq!(cache.get(1).unwrap().raw(), "b");
        assert_eq!(cache.get(2).unwrap().raw(), "c");
    }

    #[test]
    fn reinserting_updates_and_refreshes() {
        let mut cache = LineCache::new(2);
        cache.insert(0, make_line("a"));
        cache.insert(1, make_line("b"));
        // Re-insert index 0 — it should be refreshed (not evicted next).
        cache.insert(0, make_line("a2"));
        cache.insert(2, make_line("c"));
        // Index 1 should have been evicted (it was the oldest).
        assert_eq!(cache.get(0).unwrap().raw(), "a2");
        assert!(cache.get(1).is_none());
        assert_eq!(cache.get(2).unwrap().raw(), "c");
    }

    #[test]
    fn get_refreshes_access_order() {
        let mut cache = LineCache::new(2);
        cache.insert(0, make_line("a"));
        cache.insert(1, make_line("b"));
        // Access index 0 via get — it should be refreshed.
        cache.get(0);
        // Insert a new entry — index 1 (the least recently used) should be evicted.
        cache.insert(2, make_line("c"));
        assert_eq!(cache.get(0).unwrap().raw(), "a");
        assert!(cache.get(1).is_none());
        assert_eq!(cache.get(2).unwrap().raw(), "c");
    }
}
