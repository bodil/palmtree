// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#![forbid(rust_2018_idioms)]
#![deny(nonstandard_style)]
#![warn(
    unreachable_pub,
    missing_debug_implementations,
    // missing_docs,
    missing_doc_code_examples
)]
#![cfg_attr(core_intrinsics, feature(core_intrinsics))]

use std::fmt::{Debug, Error, Formatter};
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    hash::{Hash, Hasher},
    iter::FromIterator,
    ops::{Add, AddAssign, Index, IndexMut, RangeBounds},
};

mod arch;
mod branch;
mod entry;
mod iter;
mod leaf;
mod search;

use branch::{node::Node, Branch};
use leaf::Leaf;

pub use entry::Entry;
pub use iter::{Iter, IterMut, MergeIter, OwnedIter};
use search::PathedPointer;
use sized_chunks::types::ChunkLength;
use typenum::{IsGreater, Unsigned, U3, U64};

#[cfg(any(test, feature = "test"))]
pub mod tests;

pub trait NodeSize: Unsigned + IsGreater<U3> {}

pub type StdPalmTree<K, V> = PalmTree<K, V, U64, U64>;

pub struct PalmTree<K, V, B, L>
where
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    size: usize,
    root: Option<Box<Branch<K, V, B, L>>>,
}

impl<K, V, B, L> Default for PalmTree<K, V, B, L>
where
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, B, L> PalmTree<K, V, B, L>
where
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    pub fn new() -> Self {
        Self {
            size: 0,
            root: None,
        }
    }
}

impl<K, V, B, L> PalmTree<K, V, B, L>
where
    K: Clone + Ord,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    /// Construct a B+-tree efficiently from an ordered iterator.
    ///
    /// This algorithm requires the results coming out of the iterator
    /// to be in sorted order, with no duplicate keys, or the resulting
    /// tree will be in a very bad state. In debug mode, this invariant
    /// will be validated and panic ensues if it isn't held.
    pub fn load<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
    {
        fn push_stack<K: Clone, V, B, L>(
            child: Box<Branch<K, V, B, L>>,
            stack: &mut Vec<Box<Branch<K, V, B, L>>>,
        ) where
            B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
            L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
        {
            let mut parent = stack
                .pop()
                .unwrap_or_else(|| Branch::new(child.height() + 1).into());
            if parent.is_full() {
                let height = parent.height();
                push_stack(parent, stack);
                parent = Box::new(Branch::new(height));
            }
            parent.push_key(child.highest().clone());
            parent.push_branch(child);
            stack.push(parent);
        }

        #[cfg(debug_assertions)]
        let mut last_record = (0, None);

        let iter = iter.into_iter();
        let mut size = 0;
        let mut stack: Vec<Box<Branch<K, V, B, L>>> = Vec::new();
        let mut parent: Box<Branch<K, V, B, L>> = Box::new(Branch::new(1));
        let mut leaf: Box<Leaf<K, V, L>> = Box::new(Leaf::new());

        // Loop over input, fill leaf, push into parent when full.
        for (key, value) in iter {
            #[cfg(debug_assertions)]
            {
                if let (last_index, Some(last_key)) = last_record {
                    if last_key >= key {
                        panic!("PalmTree::load: unordered key at index {}", last_index);
                    }
                    last_record = (last_index + 1, Some(key.clone()));
                }
            }

            if leaf.is_full() {
                // If parent is full, push it to the parent above it on the stack.
                if parent.is_full() {
                    push_stack(parent, &mut stack);
                    parent = Box::new(Branch::new(1));
                }

                parent.push_key(leaf.keys.last().unwrap().clone());
                parent.push_leaf(leaf);

                leaf = Box::new(Leaf::new());
            }

            // Push the input into the leaf.
            leaf.keys.push_back(key);
            leaf.values.push_back(value);
            size += 1;
        }

        // If the input was empty, return immediately with an empty tree.
        if size == 0 {
            return Self {
                size: 0,
                root: None,
            };
        }

        // At end of input, push last leaf into parent, as above.
        if parent.is_full() {
            push_stack(parent, &mut stack);
            parent = Box::new(Branch::new(1));
        }
        parent.push_key(leaf.keys.last().unwrap().clone());
        parent.push_leaf(leaf);

        // Push parent into the parent above it.
        push_stack(parent, &mut stack);

        // Fold parent stack into the top level parent.
        while stack.len() > 1 {
            let parent = stack.pop().unwrap();
            push_stack(parent, &mut stack);
        }

        // The root is now the only item left on the stack.
        let mut tree = Self {
            size,
            root: stack.pop(),
        };
        tree.trim_root();
        tree
    }

    // For benchmarking: lookup with a linear search instead of binary.
    pub fn get_linear(&self, key: &K) -> Option<&V> {
        if let Some(ref root) = self.root {
            root.get_linear(key)
        } else {
            None
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        if let Some(ref root) = self.root {
            root.get(key)
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if let Some(ref mut root) = self.root {
            root.get_mut(key)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> Iter<'_, K, V, B, L> {
        Iter::new(self, ..)
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, K, V, B, L> {
        IterMut::new(self, ..)
    }

    pub fn range<R>(&self, range: R) -> Iter<'_, K, V, B, L>
    where
        R: RangeBounds<K>,
    {
        Iter::new(self, range)
    }

    pub fn range_mut<R>(&mut self, range: R) -> IterMut<'_, K, V, B, L>
    where
        R: RangeBounds<K>,
    {
        IterMut::new(self, range)
    }

    pub fn entry<'a>(&'a mut self, key: K) -> Entry<'a, K, V, B, L> {
        Entry::new(self, key)
    }

    fn split_root(root: &mut Box<Branch<K, V, B, L>>) {
        let old_root = std::mem::replace(root, Branch::new(root.height() + 1).into());
        let (left, right) = old_root.split();
        root.push_key(left.highest().clone());
        root.push_key(right.highest().clone());
        root.push_branch(left);
        root.push_branch(right);
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.entry(key) {
            Entry::Occupied(mut entry) => Some(entry.insert(value)),
            Entry::Vacant(entry) => {
                entry.insert(value);
                None
            }
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<(K, V)> {
        if let Ok(path) =
            PathedPointer::<&mut (K, V), _, _, _, _>::exact_key(self.root.as_mut()?, key)
        {
            self.size -= 1;
            Some(unsafe { path.remove() })
        } else {
            None
        }
    }

    pub fn remove_lowest(&mut self) -> Option<(K, V)> {
        if self.is_empty() {
            None
        } else {
            let path = PathedPointer::<&mut (K, V), _, _, _, _>::lowest(self.root.as_mut()?);
            self.size -= 1;
            Some(unsafe { path.remove() })
        }
    }

    pub fn remove_highest(&mut self) -> Option<(K, V)> {
        if self.is_empty() {
            None
        } else {
            let path = PathedPointer::<&mut (K, V), _, _, _, _>::highest(self.root.as_mut()?);
            self.size -= 1;
            Some(unsafe { path.remove() })
        }
    }

    fn merge_left_from(
        left: impl Iterator<Item = (K, V)>,
        right: impl Iterator<Item = (K, V)>,
    ) -> impl Iterator<Item = (K, V)> {
        MergeIter::merge(
            left,
            right,
            |(left, _), (right, _)| left > right,
            |(left, _), (right, _)| left == right,
        )
    }

    fn merge_right_from(
        left: impl Iterator<Item = (K, V)>,
        right: impl Iterator<Item = (K, V)>,
    ) -> impl Iterator<Item = (K, V)> {
        MergeIter::merge(
            left,
            right,
            |(left, _), (right, _)| left >= right,
            |(left, _), (right, _)| left == right,
        )
    }

    pub fn merge_left_iter(left: Self, right: Self) -> impl Iterator<Item = (K, V)> {
        Self::merge_left_from(left.into_iter(), right.into_iter())
    }

    pub fn merge_left(left: Self, right: Self) -> Self {
        Self::load(Self::merge_left_iter(left, right))
    }

    pub fn merge_right_iter(left: Self, right: Self) -> impl Iterator<Item = (K, V)> {
        Self::merge_right_from(left.into_iter(), right.into_iter())
    }

    pub fn merge_right(left: Self, right: Self) -> Self {
        Self::load(Self::merge_right_iter(left, right))
    }

    pub fn append_left(&mut self, other: Self) {
        let root = self.root.take();
        if root.is_some() {
            let left = OwnedIter::new(root, self.size);
            let right = other.into_iter();
            *self = Self::load(Self::merge_left_from(left, right));
        } else {
            *self = other;
        }
    }

    pub fn append_right(&mut self, other: Self) {
        let root = self.root.take();
        if root.is_some() {
            let left = OwnedIter::new(root, self.size);
            let right = other.into_iter();
            *self = Self::load(Self::merge_right_from(left, right));
        } else {
            *self = other;
        }
    }

    fn trim_root(&mut self) {
        if let Some(ref mut root) = self.root {
            // If a branch bearing root only has one child, we can replace the root with that child.
            while root.has_branches() && root.len() == 1 {
                *root = root.remove_last_branch();
            }
        }
    }
}

#[cfg(feature = "tree_debug")]
impl<K, V, B, L> Debug for PalmTree<K, V, B, L>
where
    K: Debug,
    V: Debug,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match &self.root {
            None => write!(f, "EmptyTree"),
            Some(root) => root.fmt(f),
        }
    }
}

#[cfg(not(feature = "tree_debug"))]
impl<K, V, B, L> Debug for PalmTree<K, V, B, L>
where
    K: Clone + Ord + Debug,
    V: Debug,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K, V, B, L> Clone for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    V: Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            size: self.size,
        }
    }
}

impl<K, V, B, L> FromIterator<(K, V)> for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut out = Self::new();
        for (key, value) in iter {
            out.insert(key, value);
        }
        out
    }
}

impl<'a, K, V, B, L> Index<&'a K> for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    type Output = V;

    fn index(&self, index: &K) -> &Self::Output {
        self.get(index).expect("no entry found for key")
    }
}

impl<'a, K, V, B, L> IndexMut<&'a K> for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn index_mut(&mut self, index: &K) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for key")
    }
}

impl<K, V, B, L> PartialEq for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    V: PartialEq,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<K, V, B, L> Eq for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    V: Eq,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
}

impl<K, V, B, L> PartialOrd for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    V: PartialOrd,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<K, V, B, L> Ord for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    V: Ord,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<K, V, B, L> Extend<(K, V)> for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<'a, K, V, B, L> Extend<(&'a K, &'a V)> for PalmTree<K, V, B, L>
where
    K: 'a + Ord + Copy,
    V: 'a + Copy,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn extend<I: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k.clone(), v.clone());
        }
    }
}

impl<K, V, B, L> Add for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self::merge_right(self, other)
    }
}

impl<K, V, B, L> AddAssign for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn add_assign(&mut self, other: Self) {
        self.append_right(other)
    }
}

impl<'a, K, V, B, L, B2, L2> Add<&'a PalmTree<K, V, B2, L2>> for PalmTree<K, V, B, L>
where
    K: Ord + Copy,
    V: Copy,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
    B2: ChunkLength<K> + ChunkLength<Node<K, V, B2, L2>> + IsGreater<U3>,
    L2: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    type Output = Self;

    fn add(self, other: &PalmTree<K, V, B2, L2>) -> Self::Output {
        Self::load(Self::merge_right_from(
            self.into_iter(),
            other.iter().map(|(k, v)| (k.clone(), v.clone())),
        ))
    }
}

impl<'a, K, V, B, L, B2, L2> AddAssign<&'a PalmTree<K, V, B2, L2>> for PalmTree<K, V, B, L>
where
    K: Ord + Copy,
    V: Copy,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
    B2: ChunkLength<K> + ChunkLength<Node<K, V, B2, L2>> + IsGreater<U3>,
    L2: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn add_assign(&mut self, other: &'a PalmTree<K, V, B2, L2>) {
        let root = self.root.take();
        if root.is_none() {
            *self = Self::load(other.iter().map(|(k, v)| (*k, *v)));
        } else {
            *self = Self::load(Self::merge_right_from(
                OwnedIter::new(root, self.size),
                other.iter().map(|(k, v)| (k.clone(), v.clone())),
            ))
        }
    }
}

impl<K, V, B, L> Hash for PalmTree<K, V, B, L>
where
    K: Ord + Clone + Hash,
    V: Hash,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        for entry in self {
            entry.hash(state);
        }
    }
}

impl<'a, K, V, B, L> IntoIterator for &'a PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V, B, L>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, B, L> IntoIterator for &'a mut PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V, B, L>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V, B, L> IntoIterator for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    type Item = (K, V);
    type IntoIter = OwnedIter<K, V, B, L>;
    fn into_iter(self) -> Self::IntoIter {
        OwnedIter::new(self.root, self.size)
    }
}

impl<K, V, B, L> From<BTreeMap<K, V>> for PalmTree<K, V, B, L>
where
    K: Ord + Clone,
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn from(map: BTreeMap<K, V>) -> Self {
        Self::load(map.into_iter())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lookup_empty() {
        let tree: StdPalmTree<usize, usize> = PalmTree::new();
        assert_eq!(None, tree.get(&1337));
    }

    #[test]
    fn lookup_single() {
        let mut tree: StdPalmTree<usize, usize> = PalmTree::new();
        tree.insert(1337, 31337);
        assert_eq!(None, tree.get(&1336));
        assert_eq!(Some(&31337), tree.get(&1337));
        assert_eq!(None, tree.get(&1338));
    }

    #[test]
    fn insert_in_sequence() {
        let mut tree: StdPalmTree<usize, usize> = PalmTree::new();
        let iters = 131_072;
        for i in 0..iters {
            tree.insert(i, i);
            // println!("{:?}", tree);
        }
        // println!("{:?}", tree);
        for i in 0..iters {
            assert_eq!(Some(&i), tree.get(&i));
        }
    }

    #[test]
    fn load_from_ordered_stream() {
        let size = 131_072;
        let tree: StdPalmTree<usize, usize> = PalmTree::load((0..size).map(|i| (i, i)));
        // println!("{:?}", tree);
        for i in 0..size {
            assert_eq!(Some(&i), tree.get(&i));
        }
    }

    #[test]
    fn delete_delete_delete() {
        let mut tree: StdPalmTree<usize, usize> = PalmTree::load((0..131_072).map(|i| (i, i)));
        for i in 31337..41337 {
            assert_eq!(Some((i, i)), tree.remove(&i));
            assert_eq!(None, tree.remove(&i));
        }
    }

    #[test]
    fn small_delete() {
        let mut tree: StdPalmTree<usize, usize> = PalmTree::load((0..64).map(|i| (i, i)));
        assert_eq!(Some((0, 0)), tree.remove(&0));
        assert_eq!(None, tree.remove(&0));
    }

    #[test]
    fn insert_into_emptied_tree() {
        let mut tree: StdPalmTree<u8, u8> = PalmTree::new();
        tree.insert(0, 0);
        tree.remove(&0);
        tree.insert(0, 0);
        tree.insert(10, 10);

        let result: Vec<(u8, u8)> = tree.iter().map(|(k, v)| (*k, *v)).collect();
        let expected: Vec<(u8, u8)> = vec![(0, 0), (10, 10)];
        assert_eq!(expected, result);
    }
}
