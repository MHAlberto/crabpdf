use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

/// A byte-budgeted LRU. Oversized entries are returned to the caller but not retained.
pub struct ByteCache<K, V> {
    entries: HashMap<K, (Arc<V>, usize, u64)>,
    budget: usize,
    bytes: usize,
    clock: u64,
}

impl<K: Eq + Hash + Clone, V> ByteCache<K, V> {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            budget,
            bytes: 0,
            clock: 0,
        }
    }
    pub fn get(&mut self, key: &K) -> Option<Arc<V>> {
        self.clock = self.clock.wrapping_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.2 = self.clock;
        Some(Arc::clone(&entry.0))
    }
    pub fn insert(&mut self, key: K, value: Arc<V>, size: usize) {
        if let Some((_, old_size, _)) = self.entries.remove(&key) {
            self.bytes -= old_size;
        }
        if size > self.budget {
            return;
        }
        while self.bytes.saturating_add(size) > self.budget {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, _, age))| age)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some((_, old_size, _)) = self.entries.remove(&oldest) {
                self.bytes -= old_size;
            }
        }
        self.clock = self.clock.wrapping_add(1);
        self.bytes += size;
        self.entries.insert(key, (value, size, self.clock));
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        self.entries.retain(|key, (_, size, _)| {
            if keep(key) {
                true
            } else {
                self.bytes -= *size;
                false
            }
        });
    }
    pub fn remove(&mut self, key: &K) {
        if let Some((_, size, _)) = self.entries.remove(key) {
            self.bytes -= size;
        }
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }
    pub fn most_recent(&mut self, mut matches: impl FnMut(&K) -> bool) -> Option<Arc<V>> {
        let key = self
            .entries
            .iter()
            .filter(|(key, _)| matches(key))
            .max_by_key(|(_, (_, _, age))| age)
            .map(|(key, _)| key.clone())?;
        self.get(&key)
    }
    /// Prefer the highest matching content version, then its last use.
    pub fn best_match(&mut self, mut score: impl FnMut(&K) -> Option<u64>) -> Option<(K, Arc<V>)> {
        let key = self
            .entries
            .iter()
            .filter_map(|(key, (_, _, age))| score(key).map(|score| (key, (score, *age))))
            .max_by_key(|(_, rank)| *rank)
            .map(|(key, _)| key.clone())?;
        self.get(&key).map(|value| (key, value))
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evicts_by_use_not_insertion_and_respects_budget() {
        let mut cache = ByteCache::new(8);
        cache.insert(1, Arc::new(10), 4);
        cache.insert(2, Arc::new(20), 4);
        assert_eq!(*cache.get(&1).unwrap(), 10);
        cache.insert(3, Arc::new(30), 4);
        assert!(cache.get(&2).is_none());
        assert!(cache.get(&1).is_some());
        cache.insert(4, Arc::new(40), 100);
        assert_eq!(cache.bytes(), 8);
        cache.retain(|key| *key == 3);
        assert_eq!(cache.bytes(), 4);
    }
}
