use crate::{branch::Branch, search::PathedPointer};
use std::{
    fmt::{Debug, Formatter},
    iter::FusedIterator,
};

pub struct OwnedIter<K, V> {
    tree: Option<Box<Branch<K, V>>>,
    left: PathedPointer<(), K, V>,
    right: PathedPointer<(), K, V>,
    remaining: usize,
}

impl<K, V> OwnedIter<K, V>
where
    K: Clone + Ord,
{
    pub(crate) fn new(tree: Option<Box<Branch<K, V>>>, remaining: usize) -> Self {
        if let Some(ref root) = tree {
            Self {
                left: PathedPointer::lowest(&root),
                right: PathedPointer::highest(&root),
                tree,
                remaining,
            }
        } else {
            Self {
                tree: None,
                left: PathedPointer::null(),
                right: PathedPointer::null(),
                remaining,
            }
        }
    }
}

impl<K, V> Iterator for OwnedIter<K, V>
where
    K: Clone + Ord,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.tree.is_none() {
            return None;
        }
        loop {
            let leaf = match unsafe { self.left.deref_mut_leaf() } {
                None => return None,
                Some(leaf) => leaf,
            };
            if leaf.keys.is_empty() {
                unsafe { self.left.step_forward() };
            } else {
                self.remaining -= 1;
                return Some((leaf.keys.pop_front(), leaf.values.pop_front()));
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<K, V> DoubleEndedIterator for OwnedIter<K, V>
where
    K: Clone + Ord,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.tree.is_none() {
            return None;
        }
        loop {
            let leaf = match unsafe { self.right.deref_mut_leaf() } {
                None => return None,
                Some(leaf) => leaf,
            };
            if leaf.keys.is_empty() {
                unsafe { self.left.step_back() };
            } else {
                self.remaining -= 1;
                return Some((leaf.keys.pop_back(), leaf.values.pop_back()));
            }
        }
    }
}

impl<K, V> ExactSizeIterator for OwnedIter<K, V> where K: Clone + Ord {}
impl<K, V> FusedIterator for OwnedIter<K, V> where K: Clone + Ord {}

impl<K, V> Debug for OwnedIter<K, V>
where
    K: Ord + Clone + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OwnedIter")
    }
}
