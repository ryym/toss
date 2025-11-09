use std::{
    collections::{HashMap, hash_map},
    hash::Hash,
    ptr,
};

/// A simple and naive LRU cache using raw pointers.
pub(crate) struct LruCache<K, V> {
    capacity: usize,
    map: HashMap<K, *mut Node<K, V>>,
    head: *mut Node<K, V>,
    tail: Tail<K, V>,
}

impl<K: Hash + Eq + Copy, V> LruCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            panic!("capacity must be greater than 0");
        }
        let head = Box::into_raw(Box::new(Node::Head(ptr::null_mut())));
        let tail = Box::into_raw(Box::new(Node::Tail(ptr::null_mut())));
        unsafe {
            *(*head).next_mut() = tail;
            *(*tail).prev_mut() = head;
        }
        Self {
            capacity,
            map: HashMap::with_capacity(capacity + 1),
            head,
            tail: Tail {
                tail,
                last_detached: None,
            },
        }
    }

    pub fn last_removed(&mut self) -> Option<(K, V)> {
        self.remove_last_detached_entry();
        self.tail.last_detached.take()
    }

    // When a new entry is inserted via Entry API, it is difficult to remove
    // the least used entry due to the Rust's mutable reference rule. So instead
    // this class removes the detached entry in the any next method call using this.
    fn remove_last_detached_entry(&mut self) {
        if let Some((key, _)) = &self.tail.last_detached {
            self.map.remove(key);
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.remove_last_detached_entry();
        let node = match self.map.get(key) {
            None => return None,
            Some(node) => *node,
        };
        Node::link(self.head, node);
        unsafe { Some((*node).value_mut()) }
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.remove_last_detached_entry();
        match self.map.entry(key) {
            hash_map::Entry::Occupied(mut entry) => {
                let node = *entry.get_mut();
                unsafe {
                    *(*node).value_mut() = value;
                }
                Node::link(self.head, node);
            }
            hash_map::Entry::Vacant(entry) => {
                let node = Box::into_raw(Box::new(Node::new_value(key, value)));
                entry.insert(node);
                Node::link(self.head, node);
                if self.capacity < self.map.len() {
                    self.tail.remove();
                }
            }
        }
    }

    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        self.remove_last_detached_entry();
        let is_capacity_full = self.capacity <= self.map.len();
        match self.map.entry(key) {
            hash_map::Entry::Vacant(entry) => Entry::Vacant(VacantEntry {
                key,
                entry,
                head: &mut self.head,
                tail: &mut self.tail,
                is_capacity_full,
            }),
            hash_map::Entry::Occupied(entry) => Entry::Occupied(OccupiedEntry {
                entry,
                head: &mut self.head,
            }),
        }
    }

    pub fn to_vec(&self) -> Vec<&V> {
        let mut vec = Vec::<&V>::with_capacity(self.map.len());
        unsafe {
            let mut node = *(*self.head).next_mut();
            loop {
                if matches!(*node, Node::Tail(_)) {
                    break;
                }
                vec.push((*node).value_mut());
                node = *(*node).next_mut();
            }
        }
        vec
    }
}

impl<K, V> Drop for LruCache<K, V> {
    fn drop(&mut self) {
        let mut node = self.head;
        unsafe {
            loop {
                if matches!(*node, Node::Tail(_)) {
                    drop(Box::from_raw(node));
                    break;
                }
                let next_node = *(*node).next_mut();
                drop(Box::from_raw(node));
                node = next_node;
            }
        }
    }
}

struct Tail<K, V> {
    tail: *mut Node<K, V>,
    last_detached: Option<(K, V)>,
}

impl<K: Copy, V> Tail<K, V> {
    fn remove(&mut self) {
        let node = unsafe {
            let node = *(*self.tail).prev_mut();
            if matches!(*node, Node::Head(_)) {
                panic!("cannot remove tail while empty");
            }
            (*node).detach();
            Box::from_raw(node)
        };
        self.last_detached = Some((*node.key(), node.into_value()));
    }
}

pub(crate) enum Entry<'a, K, V> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

pub(crate) struct OccupiedEntry<'a, K, V> {
    entry: hash_map::OccupiedEntry<'a, K, *mut Node<K, V>>,
    head: &'a mut *mut Node<K, V>,
}

impl<'a, K: Hash + Eq + Copy, V> OccupiedEntry<'a, K, V> {
    pub fn into_mut(self) -> &'a mut V {
        let node = self.entry.into_mut();
        Node::link(*self.head, *node);
        unsafe { (**node).value_mut() }
    }
}

pub(crate) struct VacantEntry<'a, K, V> {
    key: K,
    entry: hash_map::VacantEntry<'a, K, *mut Node<K, V>>,
    head: &'a mut *mut Node<K, V>,
    tail: &'a mut Tail<K, V>,
    is_capacity_full: bool,
}

impl<'a, K: Hash + Eq + Copy, V> VacantEntry<'a, K, V> {
    pub fn insert(self, value: V) -> &'a mut V {
        let node = Box::into_raw(Box::new(Node::new_value(self.key, value)));
        Node::link(*self.head, node);
        if self.is_capacity_full {
            self.tail.remove();
        }
        unsafe {
            let node = self.entry.insert(node);
            (**node).value_mut()
        }
    }
}

enum Node<K, V> {
    Value(Value<K, V>),
    Head(*mut Node<K, V>),
    Tail(*mut Node<K, V>),
}

struct Value<K, V> {
    key: K,
    value: V,
    prev: *mut Node<K, V>,
    next: *mut Node<K, V>,
}

impl<K, V> Node<K, V> {
    fn link(node: *mut Self, new: *mut Self) {
        // [node]-[next] => [node]-[new]-[next]
        unsafe {
            (*new).detach();
            let next_ref = (*node).next_mut();
            *(*new).prev_mut() = node;
            *(*new).next_mut() = *next_ref;
            *(**next_ref).prev_mut() = new;
            *next_ref = new;
        }
    }

    fn new_value(key: K, value: V) -> Self {
        Self::Value(Value {
            key,
            value,
            prev: ptr::null_mut(),
            next: ptr::null_mut(),
        })
    }

    fn key(&self) -> &K {
        match self {
            Self::Value(v) => &v.key,
            Self::Head(_) => panic!("key is called for head"),
            Self::Tail(_) => panic!("key is called for tail"),
        }
    }

    fn value_mut(&mut self) -> &mut V {
        match self {
            Self::Value(v) => &mut v.value,
            Self::Head(_) => panic!("value is called for head"),
            Self::Tail(_) => panic!("value is called for tail"),
        }
    }

    fn into_value(self) -> V {
        match self {
            Self::Value(v) => v.value,
            Self::Head(_) => panic!("value is called for head"),
            Self::Tail(_) => panic!("value is called for tail"),
        }
    }

    fn prev_mut(&mut self) -> &mut *mut Node<K, V> {
        match self {
            Self::Value(v) => &mut v.prev,
            Self::Head(_) => panic!("prev of head accessed"),
            Self::Tail(t) => t,
        }
    }

    fn next_mut(&mut self) -> &mut *mut Node<K, V> {
        match self {
            Self::Value(v) => &mut v.next,
            Self::Head(h) => h,
            Self::Tail(_) => panic!("next of tail accessed"),
        }
    }

    fn detach(&mut self) {
        let prev = *self.prev_mut();
        let next = *self.next_mut();
        unsafe {
            if !prev.is_null() {
                *(*prev).next_mut() = *self.next_mut();
            }
            if !next.is_null() {
                *(*next).prev_mut() = *self.prev_mut();
            }
        }
        *self.prev_mut() = ptr::null_mut();
        *self.next_mut() = ptr::null_mut();
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use crate::minilib::lru_cache::{Entry, LruCache};

    #[test]
    fn insert_items() {
        let mut lru = LruCache::<usize, i16>::new(3);
        lru.insert(1, -1);
        lru.insert(2, -2);
        assert_eq!(lru.to_vec(), vec![&-2, &-1]);

        lru.insert(3, -3);
        assert_eq!(lru.to_vec(), vec![&-3, &-2, &-1]);
        lru.insert(4, -4);
        assert_eq!(lru.to_vec(), vec![&-4, &-3, &-2]);
    }

    #[test]
    fn get_items_with_updating_order() {
        let mut lru = LruCache::<usize, i16>::new(3);
        lru.insert(1, -1);
        lru.insert(2, -2);
        assert_eq!(lru.to_vec(), vec![&-2, &-1]);

        assert_eq!(lru.get(&2), Some(&-2));
        assert_eq!(lru.get(&1), Some(&-1));
        assert_eq!(lru.to_vec(), vec![&-1, &-2]);

        assert_eq!(lru.get(&3), None);
    }

    #[test]
    fn get_and_insert_via_entry_api() {
        let mut lru = LruCache::<usize, i16>::new(3);
        lru.insert(1, -1);
        lru.insert(2, -2);
        assert_eq!(lru.to_vec(), vec![&-2, &-1]);

        match lru.entry(1) {
            Entry::Occupied(entry) => {
                assert_eq!(entry.into_mut(), &mut -1);
            }
            Entry::Vacant(_) => panic!("entry 1 should exist"),
        }
        assert_eq!(lru.to_vec(), vec![&-1, &-2]);

        match lru.entry(3) {
            Entry::Occupied(_) => panic!("entry 3 should not exist"),
            Entry::Vacant(entry) => entry.insert(-3),
        };
        assert_eq!(lru.to_vec(), vec![&-3, &-1, &-2]);

        match lru.entry(4) {
            Entry::Occupied(_) => panic!("entry 4 should not exist"),
            Entry::Vacant(entry) => entry.insert(-4),
        };
        assert_eq!(lru.to_vec(), vec![&-4, &-3, &-1]);
        assert_eq!(lru.last_removed(), Some((2, -2)));
    }

    #[test]
    fn drop_all_items() {
        struct Item(Rc<Cell<bool>>);
        impl Item {
            fn new() -> (Self, Rc<Cell<bool>>) {
                let dropped = Rc::new(Cell::new(false));
                let foo = Self(dropped.clone());
                (foo, dropped)
            }
        }

        impl Drop for Item {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let (item1, dropped1) = Item::new();
        let (item2, dropped2) = Item::new();
        let (item3, dropped3) = Item::new();
        let (item4, dropped4) = Item::new();
        let flags = || {
            [
                dropped1.get(),
                dropped2.get(),
                dropped3.get(),
                dropped4.get(),
            ]
        };

        {
            let mut lru = LruCache::<usize, Item>::new(2);
            lru.insert(1, item1);
            lru.insert(2, item2);
            assert_eq!(flags(), [false, false, false, false]);

            // This removes the first item but it is still alive.
            lru.insert(3, item3);
            assert_eq!(flags(), [false, false, false, false]);

            // But once we consume it, the first item will be dropped.
            assert_eq!(lru.last_removed().map(|t| t.0), Some(1));
            assert_eq!(flags(), [true, false, false, false]);

            // The second item is detached but alive.
            lru.insert(4, item4);
            assert_eq!(flags(), [true, false, false, false]);
        }
        // If the LruCache itself is dropped, all items are dropped.
        assert_eq!(flags(), [true, true, true, true]);
    }
}
