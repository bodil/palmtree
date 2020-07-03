use super::paths_from_range;
use crate::{branch::node::Node, search::PathedPointer, PalmTree};
use sized_chunks::types::ChunkLength;
use std::{
    cmp::Ordering,
    fmt::{Debug, Error, Formatter},
    iter::FusedIterator,
    ops::RangeBounds,
};
use typenum::{IsGreater, U3};

pub struct Iter<'a, K, V, B, L>
where
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    left: PathedPointer<&'a (K, V), K, V, B, L>,
    right: PathedPointer<&'a (K, V), K, V, B, L>,
}

impl<'a, K, V, B, L> Clone for Iter<'a, K, V, B, L>
where
    K: Clone + Ord,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn clone(&self) -> Self {
        Self {
            left: self.left.clone(),
            right: self.right.clone(),
        }
    }
}

impl<'a, K, V, B, L> Iter<'a, K, V, B, L>
where
    K: Clone + Ord,
    B: 'a + ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: 'a + ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn null() -> Self {
        Self {
            left: PathedPointer::null(),
            right: PathedPointer::null(),
        }
    }

    pub(crate) fn new<R>(tree: &'a PalmTree<K, V, B, L>, range: R) -> Self
    where
        R: RangeBounds<K>,
    {
        if let Some((left, right)) = paths_from_range(tree, range) {
            Self { left, right }
        } else {
            Self::null()
        }
    }

    fn step_forward(&mut self) {
        let result = unsafe { self.left.step_forward() };
        debug_assert!(result);
    }

    fn step_back(&mut self) {
        let result = unsafe { self.right.step_back() };
        debug_assert!(result);
    }

    fn left(&self) -> &'a PathedPointer<&'a (), K, V, B, L> {
        unsafe { &*(&self.left as *const _ as *const PathedPointer<&'a (), K, V, B, L>) }
    }

    fn right(&self) -> &'a PathedPointer<&'a (), K, V, B, L> {
        unsafe { &*(&self.right as *const _ as *const PathedPointer<&'a (), K, V, B, L>) }
    }

    fn left_key(&self) -> Option<&'a K> {
        unsafe { self.left().key() }
    }

    fn left_value(&self) -> Option<&'a V> {
        unsafe { self.left().value() }
    }

    fn right_key(&self) -> Option<&'a K> {
        unsafe { self.right().key() }
    }

    fn right_value(&self) -> Option<&'a V> {
        unsafe { self.right().value() }
    }
}

impl<'a, K, V, B, L> Iterator for Iter<'a, K, V, B, L>
where
    K: Clone + Ord,
    B: 'a + ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: 'a + ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let left_key = self.left_key()?;
        let right_key = self.right_key()?;
        // If left key is greather than right key, we're done.
        let cmp = left_key.cmp(right_key);
        if cmp == Ordering::Greater {
            self.left.clear();
            self.right.clear();
            return None;
        }
        let value = self.left_value().unwrap();
        if cmp == Ordering::Equal {
            self.left.clear();
            self.right.clear();
        } else {
            self.step_forward();
        }
        Some((left_key, value))
    }
}

impl<'a, K, V, B, L> DoubleEndedIterator for Iter<'a, K, V, B, L>
where
    K: Clone + Ord,
    B: 'a + ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: 'a + ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let left_key = self.left_key()?;
        let right_key = self.right_key()?;
        // If left key is greather than right key, we're done.
        let cmp = left_key.cmp(right_key);
        if cmp == Ordering::Greater {
            self.left.clear();
            self.right.clear();
            return None;
        }
        let value = self.right_value().unwrap();
        if cmp == Ordering::Equal {
            self.left.clear();
            self.right.clear();
        } else {
            self.step_back();
        }
        Some((right_key, value))
    }
}

impl<'a, K, V, B, L> FusedIterator for Iter<'a, K, V, B, L>
where
    K: Clone + Ord,
    B: 'a + ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: 'a + ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
}

impl<'a, K, V, B, L> Debug for Iter<'a, K, V, B, L>
where
    K: Clone + Ord + Debug,
    V: Debug,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_map().entries(self.clone()).finish()
    }
}
