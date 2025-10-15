use std::{
    collections::BTreeSet, 
    ops::{Deref, DerefMut}, 
    sync::{
        atomic::{AtomicUsize, Ordering}, 
        Arc
    }
};

use dashmap::DashMap;

struct RoundRobinSet<T> {
    inner: BTreeSet<T>,
    counter: AtomicUsize,
}

impl<T> Default for RoundRobinSet<T> 
where 
    T: std::cmp::Eq + std::cmp::Ord
{
    fn default() -> Self {
        Self { 
            inner: Default::default(), 
            counter: Default::default() 
        }
    }
}

impl<T> Deref for RoundRobinSet<T> 
where 
    T: std::cmp::Eq + std::cmp::Ord
{
    type Target = BTreeSet<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for RoundRobinSet<T> 
where 
    T: std::cmp::Eq + std::cmp::Ord
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> RoundRobinSet<T> 
where 
    T: Clone + std::cmp::Eq + std::cmp::Ord 
{
    fn next(&self) -> Option<T> {
        if self.inner.is_empty() {
            return None;
        }
        
        // Get current count and increment atomically
        let current = self.counter.fetch_add(1, Ordering::Relaxed);
        let index = current % self.inner.len();
        
        // Use iterator to get nth element since BTreeSet is ordered
        self.inner.iter().nth(index).cloned()
    }

    // Create a new RoundRobinSet from BTreeSet
    fn from_set(set: BTreeSet<T>) -> Self {
        Self {
            inner: set,
            counter: AtomicUsize::new(0),
        }
    }
}

#[derive(Default)]
pub struct RoundRobinDashMap<T: Clone> {
    inner: DashMap<String, Arc<RoundRobinSet<T>>>,
}

impl<T> RoundRobinDashMap<T> 
where 
    T: Clone + std::cmp::Eq + std::cmp::Ord + Send + Sync + 'static
{
    pub fn insert(&self, key: String, value: T) {
        self.inner
            .entry(key)
            .and_modify(|entry| {
                // Clone value here since we need it in multiple places
                let value = value.clone();
                if let Some(mut_entry) = Arc::get_mut(entry) {
                    mut_entry.inner.insert(value);
                } else {
                    // If there are multiple references, create a new set with existing values
                    let mut new_set = entry.inner.clone();
                    new_set.insert(value);
                    *entry = Arc::new(RoundRobinSet {
                        inner: new_set,
                        counter: AtomicUsize::new(0),
                    });
                }
            })
            .or_insert_with(|| {
                // If key doesn't exist, create a new set containing only the new value
                // This avoids unnecessary allocations and cloning
                let mut set = BTreeSet::new();
                set.insert(value);
                Arc::new(RoundRobinSet::from_set(set))
            });
    }

    pub fn remove(&self, key: String, value: T) -> bool {
        if let Some(mut entry) = self.inner.get_mut(&key) {
            if let Some(round_robin) = Arc::get_mut(entry.value_mut()) {
                round_robin.inner.remove(&value)
            } else {
                // If there are multiple references, create new set
                let mut new_set = entry.inner.clone();
                let removed = new_set.remove(&value);
                if removed {
                    *entry.value_mut() = Arc::new(RoundRobinSet {
                        inner: new_set,
                        counter: AtomicUsize::new(0),
                    });
                }
                removed
            }
        } else {
            false
        }
    }

    pub fn get_round_robin(&self, key: &str) -> Option<T> {
        let entry = self.inner.get(key)?;
        entry.next()
    }

    pub fn update(&self, key: &str, new_set: BTreeSet<T>) -> bool {
        self.inner.insert(key.to_string(), Arc::new(RoundRobinSet::from_set(new_set)));
        true
    }
    pub fn contains_key(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    pub fn keys(&self) -> Vec<String> {
        self.inner.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// Example usage
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_robin() {
        let map = RoundRobinDashMap::<String>::default();
        
        // Insert test data
        map.insert("test".to_string(), "node1".to_string());
        map.insert("test".to_string(), "node2".to_string());
        
        // Test round robin selection
        let first = map.get_round_robin("test");
        let second = map.get_round_robin("test");
        assert!(first.is_some());
        assert!(second.is_some());
        assert_ne!(first, second);
    }
}