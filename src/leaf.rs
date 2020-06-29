use crate::types::LeafSize;
use sized_chunks::Chunk;
use std::fmt::{Debug, Error, Formatter};

/// A leaf node contains an ordered sequence of direct mappings from keys to values.
#[derive(Clone)]
pub(crate) struct Leaf<K, V> {
    pub(crate) keys: Chunk<K, LeafSize>,
    pub(crate) values: Chunk<V, LeafSize>,
}

impl<K, V> Leaf<K, V> {
    pub(crate) fn new() -> Self {
        Leaf {
            keys: Chunk::new(),
            values: Chunk::new(),
        }
    }

    pub(crate) fn unit(key: K, value: V) -> Self {
        Leaf {
            keys: Chunk::unit(key),
            values: Chunk::unit(value),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.keys.is_full()
    }

    pub(crate) fn highest(&self) -> &K {
        self.keys.last().unwrap()
    }

    pub(crate) fn keys(&self) -> &[K] {
        &self.keys
    }

    pub(crate) fn split(mut self: Box<Self>) -> (Box<Leaf<K, V>>, Box<Leaf<K, V>>) {
        let half = self.keys.len() / 2;
        let left = Box::new(Leaf {
            keys: Chunk::from_front(&mut self.keys, half),
            values: Chunk::from_front(&mut self.values, half),
        });
        (left, self)
    }
}

impl<K, V> Leaf<K, V>
where
    K: Clone + Ord,
{
    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.keys
            .binary_search(key)
            .ok()
            .map(|index| &self.values[index])
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if let Ok(index) = self.keys.binary_search(key) {
            Some(&mut self.values[index])
        } else {
            None
        }
    }

    pub(crate) fn get_linear(&self, key: &K) -> Option<&V> {
        for (index, stored_key) in self.keys.iter().enumerate() {
            if stored_key == key {
                return Some(&self.values[index]);
            }
        }
        None
    }
}

impl<K, V> Debug for Leaf<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let pairs: Vec<_> = self.keys.iter().zip(self.values.iter()).collect();
        writeln!(f, "Leaf(len={}) {:?}", self.len(), pairs)
    }
}
