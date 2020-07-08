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
#![allow(clippy::question_mark)] // this lint makes code less readable
#![allow(clippy::large_enum_variant)] // this lint is buggy
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
mod array;
mod branch;
mod config;
mod entry;
mod iter;
mod leaf;
mod search;

use branch::Branch;
use leaf::Leaf;

pub use config::{Tree64, TreeConfig};
pub use entry::Entry;
pub use iter::{Iter, IterMut, MergeIter, OwnedIter};
use search::PathedPointer;

#[cfg(any(test, feature = "test"))]
pub mod tests;

enum InsertResult<K, V> {
    Added,
    Replaced(V),
    Full(K, V),
}

pub type StdPalmTree<K, V> = PalmTree<K, V, Tree64>;

pub struct PalmTree<K, V, C>
where
    C: TreeConfig<K, V>,
{
    size: usize,
    root: Option<Box<Branch<K, V, C>>>,
}

impl<K, V, C> Default for PalmTree<K, V, C>
where
    C: TreeConfig<K, V>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, C> PalmTree<K, V, C>
where
    C: TreeConfig<K, V>,
{
    pub fn new() -> Self {
        Self {
            size: 0,
            root: None,
        }
    }
}

impl<K, V, C> PalmTree<K, V, C>
where
    K: Clone + Ord,
    C: TreeConfig<K, V>,
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
        fn push_stack<K: Clone, V, C>(
            child: Box<Branch<K, V, C>>,
            stack: &mut Vec<Box<Branch<K, V, C>>>,
        ) where
            C: TreeConfig<K, V>,
        {
            let mut parent = stack.pop().unwrap_or_else(|| Branch::new(true).into());
            if parent.is_full() {
                push_stack(parent, stack);
                parent = Box::new(Branch::new(true));
            }
            parent.push_branch(child.highest().clone(), child);
            stack.push(parent);
        }

        #[cfg(debug_assertions)]
        let mut last_record = (0, None);

        let iter = iter.into_iter();
        let mut size = 0;
        let mut stack: Vec<Box<Branch<K, V, C>>> = Vec::new();
        let mut parent: Box<Branch<K, V, C>> = Box::new(Branch::new(false));
        let mut leaf: Box<Leaf<K, V, C>> = Box::new(Leaf::new());

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
                    parent = Box::new(Branch::new(false));
                }

                parent.push_leaf(leaf.highest().clone(), leaf);

                leaf = Box::new(Leaf::new());
            }

            // Push the input into the leaf.
            unsafe { leaf.push_unchecked(key, value) };
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
            parent = Box::new(Branch::new(false));
        }
        parent.push_leaf(leaf.highest().clone(), leaf);

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

    pub fn iter(&self) -> Iter<'_, K, V, C> {
        Iter::new(self, ..)
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, K, V, C> {
        IterMut::new(self, ..)
    }

    pub fn range<R>(&self, range: R) -> Iter<'_, K, V, C>
    where
        R: RangeBounds<K>,
    {
        Iter::new(self, range)
    }

    pub fn range_mut<R>(&mut self, range: R) -> IterMut<'_, K, V, C>
    where
        R: RangeBounds<K>,
    {
        IterMut::new(self, range)
    }

    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, C> {
        Entry::new(self, key)
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
        if let Ok(path) = PathedPointer::<&mut (K, V), _, _, _>::exact_key(self.root.as_mut()?, key)
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
            let path = PathedPointer::<&mut (K, V), _, _, _>::lowest(self.root.as_mut()?);
            self.size -= 1;
            Some(unsafe { path.remove() })
        }
    }

    pub fn remove_highest(&mut self) -> Option<(K, V)> {
        if self.is_empty() {
            None
        } else {
            let path = PathedPointer::<&mut (K, V), _, _, _>::highest(self.root.as_mut()?);
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
                *root = root.remove_last_branch().1;
            }
        }
    }

    #[allow(clippy::borrowed_box)]
    fn split_root(root: &mut Box<Branch<K, V, C>>) {
        let old_root = std::mem::replace(root, Branch::new(true).into());
        let (left, right) = old_root.split();
        root.push_branch_pair(left.highest().clone(), left, right.highest().clone(), right);
    }

    pub fn insert_recursive(&mut self, key: K, value: V) -> Option<V> {
        let len = self.size;
        if let Some(ref mut root) = self.root {
            // Special case: if a tree has size 0 but there is a root, it's because
            // we removed the last entry and the root has been left allocated.
            // Tree walking algos assume the tree has no empty nodes, so we have to
            // handle this as a special case.
            if len == 0 {
                // Make sure the delete trimmed the tree properly.
                debug_assert_eq!(0, root.len());
                debug_assert!(root.has_leaves());

                root.push_leaf(key.clone(), Box::new(Leaf::unit(key, value)));
                self.size = 1;
                None
            } else {
                match root.insert(key, value) {
                    InsertResult::Added => {
                        self.size += 1;
                        None
                    }
                    InsertResult::Replaced(value) => Some(value),
                    InsertResult::Full(key, value) => {
                        // If the root is full, we need to increase the height of the tree and retry insertion,
                        // so we can split the old root.
                        let key2 = root.highest().clone();
                        let child = std::mem::replace(&mut *root, Box::new(Branch::new(true)));
                        root.push_branch(key2, child);
                        self.insert(key, value)
                    }
                }
            }
        } else {
            self.root = Some(Box::new(Branch::unit(Box::new(Leaf::unit(key, value)))));
            self.size = 1;
            None
        }
    }
}

#[cfg(feature = "tree_debug")]
impl<K, V, C> Debug for PalmTree<K, V, C>
where
    K: Debug,
    V: Debug,
    C: TreeConfig<K, V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match &self.root {
            None => write!(f, "EmptyTree"),
            Some(root) => root.fmt(f),
        }
    }
}

#[cfg(not(feature = "tree_debug"))]
impl<K, V, C> Debug for PalmTree<K, V, C>
where
    K: Clone + Ord + Debug,
    V: Debug,
    C: TreeConfig<K, V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K, V, C> Clone for PalmTree<K, V, C>
where
    K: Ord + Clone,
    V: Clone,
    C: TreeConfig<K, V>,
{
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            size: self.size,
        }
    }
}

impl<K, V, C> FromIterator<(K, V)> for PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
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

impl<'a, K, V, C> Index<&'a K> for PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    type Output = V;

    fn index(&self, index: &K) -> &Self::Output {
        self.get(index).expect("no entry found for key")
    }
}

impl<'a, K, V, C> IndexMut<&'a K> for PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    fn index_mut(&mut self, index: &K) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for key")
    }
}

impl<K, V, C> PartialEq for PalmTree<K, V, C>
where
    K: Ord + Clone,
    V: PartialEq,
    C: TreeConfig<K, V>,
{
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<K, V, C> Eq for PalmTree<K, V, C>
where
    K: Ord + Clone,
    V: Eq,
    C: TreeConfig<K, V>,
{
}

impl<K, V, C> PartialOrd for PalmTree<K, V, C>
where
    K: Ord + Clone,
    V: PartialOrd,
    C: TreeConfig<K, V>,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<K, V, C> Ord for PalmTree<K, V, C>
where
    K: Ord + Clone,
    V: Ord,
    C: TreeConfig<K, V>,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<K, V, C> Extend<(K, V)> for PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<'a, K, V, C> Extend<(&'a K, &'a V)> for PalmTree<K, V, C>
where
    K: 'a + Ord + Copy,
    V: 'a + Copy,
    C: TreeConfig<K, V>,
{
    fn extend<I: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(*k, *v);
        }
    }
}

impl<K, V, C> Add for PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self::merge_right(self, other)
    }
}

impl<K, V, C> AddAssign for PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    fn add_assign(&mut self, other: Self) {
        self.append_right(other)
    }
}

impl<'a, K, V, C, C2> Add<&'a PalmTree<K, V, C2>> for PalmTree<K, V, C>
where
    K: Ord + Copy,
    V: Copy,
    C: TreeConfig<K, V>,
    C2: TreeConfig<K, V>,
{
    type Output = Self;

    fn add(self, other: &PalmTree<K, V, C2>) -> Self::Output {
        Self::load(Self::merge_right_from(
            self.into_iter(),
            other.iter().map(|(k, v)| (*k, *v)),
        ))
    }
}

impl<'a, K, V, C, C2> AddAssign<&'a PalmTree<K, V, C2>> for PalmTree<K, V, C>
where
    K: Ord + Copy,
    V: Copy,
    C: TreeConfig<K, V>,
    C2: TreeConfig<K, V>,
{
    fn add_assign(&mut self, other: &'a PalmTree<K, V, C2>) {
        let root = self.root.take();
        if root.is_none() {
            *self = Self::load(other.iter().map(|(k, v)| (*k, *v)));
        } else {
            *self = Self::load(Self::merge_right_from(
                OwnedIter::new(root, self.size),
                other.iter().map(|(k, v)| (*k, *v)),
            ))
        }
    }
}

impl<K, V, C> Hash for PalmTree<K, V, C>
where
    K: Ord + Clone + Hash,
    V: Hash,
    C: TreeConfig<K, V>,
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

impl<'a, K, V, C> IntoIterator for &'a PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V, C>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, C> IntoIterator for &'a mut PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V, C>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V, C> IntoIterator for PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    type Item = (K, V);
    type IntoIter = OwnedIter<K, V, C>;
    fn into_iter(self) -> Self::IntoIter {
        OwnedIter::new(self.root, self.size)
    }
}

impl<K, V, C> From<BTreeMap<K, V>> for PalmTree<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
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
        }
        for i in 0..iters {
            assert_eq!(Some(&i), tree.get(&i));
        }
    }

    #[test]
    fn load_from_ordered_stream() {
        let size = 131_072;
        let tree: StdPalmTree<usize, usize> = PalmTree::load((0..size).map(|i| (i, i)));
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
