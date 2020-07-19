use crate::{branch::Branch, config::TreeConfig, pointer::Pointer, search::PathedPointer};
use std::{
    fmt::{Debug, Formatter},
    iter::FusedIterator,
};

pub struct OwnedIter<K, V, C>
where
    C: TreeConfig<K, V>,
{
    tree: Option<Pointer<Branch<K, V, C>, C::PointerKind>>,
    left: PathedPointer<(K, V), K, V, C>,
    right: PathedPointer<(K, V), K, V, C>,
    remaining: usize,
}

impl<K, V, C> OwnedIter<K, V, C>
where
    K: Clone + Ord,
    C: TreeConfig<K, V>,
{
    pub(crate) fn new(
        tree: Option<Pointer<Branch<K, V, C>, C::PointerKind>>,
        remaining: usize,
    ) -> Self {
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

impl<K, V, C> Iterator for OwnedIter<K, V, C>
where
    K: Clone + Ord,
    C: TreeConfig<K, V>,
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

impl<K, V, C> DoubleEndedIterator for OwnedIter<K, V, C>
where
    K: Clone + Ord,
    C: TreeConfig<K, V>,
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

impl<K, V, C> ExactSizeIterator for OwnedIter<K, V, C>
where
    K: Clone + Ord,
    C: TreeConfig<K, V>,
{
}
impl<K, V, C> FusedIterator for OwnedIter<K, V, C>
where
    K: Clone + Ord,
    C: TreeConfig<K, V>,
{
}

impl<K, V, C> Debug for OwnedIter<K, V, C>
where
    K: Ord + Clone + Debug,
    V: Debug,
    C: TreeConfig<K, V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OwnedIter")
    }
}
