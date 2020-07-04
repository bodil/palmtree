use crate::{array::Array, InsertResult};
use generic_array::ArrayLength;
use std::fmt::{Debug, Error, Formatter};
use typenum::{IsGreater, U3};

/// A leaf node contains an ordered sequence of direct mappings from keys to values.
pub(crate) struct Leaf<K, V, L>
where
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    length: usize,
    keys: Array<K, L>,
    values: Array<V, L>,
}

impl<K, V, L> Drop for Leaf<K, V, L>
where
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    fn drop(&mut self) {
        unsafe {
            self.keys.drop(self.length);
            self.values.drop(self.length);
        }
    }
}

impl<K, V, L> Clone for Leaf<K, V, L>
where
    K: Clone,
    V: Clone,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    fn clone(&self) -> Self {
        Self {
            length: self.length,
            keys: unsafe { self.keys.clone(self.length) },
            values: unsafe { self.values.clone(self.length) },
        }
    }
}

impl<K, V, L> Leaf<K, V, L>
where
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    pub(crate) fn new() -> Self {
        Leaf {
            length: 0,
            keys: Array::new(),
            values: Array::new(),
        }
    }

    pub(crate) fn unit(key: K, value: V) -> Self {
        Leaf {
            length: 1,
            keys: unsafe { Array::unit(key) },
            values: unsafe { Array::unit(value) },
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.length
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn is_full(&self) -> bool {
        self.len() == L::USIZE
    }

    pub(crate) fn highest(&self) -> &K {
        &self.keys()[self.len() - 1]
    }

    pub(crate) fn keys(&self) -> &[K] {
        unsafe { self.keys.deref(self.length) }
    }

    pub(crate) fn values(&self) -> &[V] {
        unsafe { self.values.deref(self.length) }
    }

    pub(crate) fn keys_mut(&mut self) -> &mut [K] {
        unsafe { self.keys.deref_mut(self.length) }
    }

    pub(crate) fn values_mut(&mut self) -> &mut [V] {
        unsafe { self.values.deref_mut(self.length) }
    }

    pub(crate) fn split(mut self: Box<Self>) -> (Box<Leaf<K, V, L>>, Box<Leaf<K, V, L>>) {
        let half = self.length / 2;
        let right = Box::new(Leaf {
            length: half,
            keys: unsafe { Array::steal_from(&mut self.keys, self.length, half) },
            values: unsafe { Array::steal_from(&mut self.values, self.length, half) },
        });
        self.length -= half;
        (self, right)
    }

    pub(crate) unsafe fn push_unchecked(&mut self, key: K, value: V) {
        self.keys.push(self.length, key);
        self.values.push(self.length, value);
        self.length += 1;
    }

    pub(crate) unsafe fn insert_unchecked(&mut self, index: usize, key: K, value: V) {
        self.keys.insert(self.length, index, key);
        self.values.insert(self.length, index, value);
        self.length += 1;
    }

    pub(crate) unsafe fn remove_unchecked(&mut self, index: usize) -> (K, V) {
        let result = (
            self.keys.remove(self.length, index),
            self.values.remove(self.length, index),
        );
        self.length -= 1;
        result
    }

    pub(crate) fn pop_back(&mut self) -> Option<(K, V)> {
        if !self.is_empty() {
            let result =
                Some(unsafe { (self.keys.pop(self.length), self.values.pop(self.length)) });
            self.length -= 1;
            result
        } else {
            None
        }
    }

    pub(crate) fn pop_front(&mut self) -> Option<(K, V)> {
        if !self.is_empty() {
            // TODO we could speed this up a lot by keeping a left index as well as a length, a la Chunk,
            // but it's only used by OwnedIterator, and it would adversely affect anything else. Think about it.
            let result = Some(unsafe {
                (
                    self.keys.remove(self.length, 0),
                    self.values.remove(self.length, 0),
                )
            });
            self.length -= 1;
            result
        } else {
            None
        }
    }
}

impl<K, V, L> Leaf<K, V, L>
where
    K: Clone + Ord,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.keys()
            .binary_search(key)
            .ok()
            .map(|index| unsafe { self.values().get_unchecked(index) })
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if let Ok(index) = self.keys().binary_search(key) {
            Some(unsafe { self.values_mut().get_unchecked_mut(index) })
        } else {
            None
        }
    }

    pub(crate) fn get_linear(&self, key: &K) -> Option<&V> {
        for (index, stored_key) in self.keys().iter().enumerate() {
            if stored_key == key {
                return Some(unsafe { self.values().get_unchecked(index) });
            }
        }
        None
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> InsertResult<K, V> {
        match self.keys().binary_search(&key) {
            Ok(index) => InsertResult::Replaced(std::mem::replace(
                unsafe { self.values_mut().get_unchecked_mut(index) },
                value,
            )),
            Err(index) => {
                if !self.is_full() {
                    unsafe { self.insert_unchecked(index, key, value) };
                    InsertResult::Added
                } else {
                    InsertResult::Full(key, value)
                }
            }
        }
    }
}

impl<K, V, L> Debug for Leaf<K, V, L>
where
    K: Debug,
    V: Debug,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let pairs: Vec<_> = self.keys().iter().zip(self.values().iter()).collect();
        writeln!(f, "Leaf(len={}) {:?}", self.len(), pairs)
    }
}
