use crate::{branch::Branch, search::PathedPointer, PalmTree};
use std::fmt::{Debug, Formatter};

pub struct OwnedIter<K, V> {
    tree: Option<Box<Branch<K, V>>>,
    left: PathedPointer<(), K, V>,
    right: PathedPointer<(), K, V>,
}

impl<K, V> OwnedIter<K, V>
where
    K: Clone + Ord,
{
    pub(crate) fn new(tree: PalmTree<K, V>) -> Self {
        let tree = tree.root;
        if let Some(ref root) = tree {
            Self {
                left: PathedPointer::lowest(&root),
                right: PathedPointer::highest(&root),
                tree,
            }
        } else {
            Self {
                tree: None,
                left: PathedPointer::null(),
                right: PathedPointer::null(),
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
                return Some((leaf.keys.pop_front(), leaf.values.pop_front()));
            }
        }
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
                return Some((leaf.keys.pop_back(), leaf.values.pop_back()));
            }
        }
    }
}

impl<K, V> Debug for OwnedIter<K, V>
where
    K: Ord + Clone + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OwnedIter")
    }
}
