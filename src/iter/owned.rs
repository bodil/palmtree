use crate::{
    branch::{node::Node, Branch},
    search::PathedPointer,
};
use generic_array::ArrayLength;
use std::{
    fmt::{Debug, Formatter},
    iter::FusedIterator,
};
use typenum::{IsGreater, U3};

pub struct OwnedIter<K, V, B, L>
where
    B: ArrayLength<K> + ArrayLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    tree: Option<Box<Branch<K, V, B, L>>>,
    left: PathedPointer<(K, V), K, V, B, L>,
    right: PathedPointer<(K, V), K, V, B, L>,
    remaining: usize,
}

impl<K, V, B, L> OwnedIter<K, V, B, L>
where
    K: Clone + Ord,
    B: ArrayLength<K> + ArrayLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    pub(crate) fn new(tree: Option<Box<Branch<K, V, B, L>>>, remaining: usize) -> Self {
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

impl<K, V, B, L> Iterator for OwnedIter<K, V, B, L>
where
    K: Clone + Ord,
    B: ArrayLength<K> + ArrayLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
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
            if leaf.is_empty() {
                unsafe { self.left.step_forward() };
            } else {
                let result = leaf.pop_front();
                self.remaining -= 1;
                return result;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<K, V, B, L> DoubleEndedIterator for OwnedIter<K, V, B, L>
where
    K: Clone + Ord,
    B: ArrayLength<K> + ArrayLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
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
            if leaf.is_empty() {
                unsafe { self.left.step_back() };
            } else {
                self.remaining -= 1;
                return leaf.pop_back();
            }
        }
    }
}

impl<K, V, B, L> ExactSizeIterator for OwnedIter<K, V, B, L>
where
    K: Clone + Ord,
    B: ArrayLength<K> + ArrayLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
}
impl<K, V, B, L> FusedIterator for OwnedIter<K, V, B, L>
where
    K: Clone + Ord,
    B: ArrayLength<K> + ArrayLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
}

impl<K, V, B, L> Debug for OwnedIter<K, V, B, L>
where
    K: Ord + Clone + Debug,
    V: Debug,
    B: ArrayLength<K> + ArrayLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OwnedIter")
    }
}
