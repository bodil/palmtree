use super::paths_from_range;
use crate::{search::PathedPointer, PalmTree};
use std::{
    cmp::Ordering,
    fmt::{Debug, Formatter},
    iter::FusedIterator,
    ops::RangeBounds,
};

pub struct IterMut<'a, K, V> {
    left: PathedPointer<&'a mut (K, V), K, V>,
    right: PathedPointer<&'a mut (K, V), K, V>,
}

impl<'a, K, V> IterMut<'a, K, V>
where
    K: Clone + Ord,
{
    fn null() -> Self {
        Self {
            left: PathedPointer::null(),
            right: PathedPointer::null(),
        }
    }

    /// Construct a mutable iterator.
    ///
    /// Here is a doctest to ensure you can't have two mutable iterators over the same tree
    /// at the same time:
    ///
    /// ```compile_fail
    /// use palmtree::PalmTree;
    /// let mut tree = PalmTree::load((0..4096).map(|i| (i, i)));
    /// let mut it1 = tree.iter_mut();
    /// let mut it2 = tree.iter_mut();
    /// assert_eq!(it1.next(), it2.next());
    /// ```
    pub(crate) fn new<R>(tree: &'a mut PalmTree<K, V>, range: R) -> Self
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

    fn left(&mut self) -> &'a mut PathedPointer<&'a mut (), K, V> {
        unsafe { &mut *(&mut self.left as *mut _ as *mut PathedPointer<&'a mut (), K, V>) }
    }

    fn right(&mut self) -> &'a mut PathedPointer<&'a mut (), K, V> {
        unsafe { &mut *(&mut self.right as *mut _ as *mut PathedPointer<&'a mut (), K, V>) }
    }

    fn left_key(&mut self) -> Option<&'a K> {
        unsafe { self.left().key() }
    }

    fn left_value(&mut self) -> Option<&'a mut V> {
        unsafe { self.left().value_mut() }
    }

    fn right_key(&mut self) -> Option<&'a K> {
        unsafe { self.right().key() }
    }

    fn right_value(&mut self) -> Option<&'a mut V> {
        unsafe { self.right().value_mut() }
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V>
where
    K: Clone + Ord,
{
    type Item = (&'a K, &'a mut V);

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

impl<'a, K, V> DoubleEndedIterator for IterMut<'a, K, V>
where
    K: 'a + Clone + Ord,
    V: 'a,
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

impl<'a, K, V> FusedIterator for IterMut<'a, K, V> where K: Clone + Ord {}

impl<'a, K, V> Debug for IterMut<'a, K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "IterMut")
    }
}
