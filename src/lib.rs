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
mod search;
mod types;

mod leaf;

mod branch;

mod iter;

use branch::Branch;
use leaf::Leaf;
use types::{InsertResult, RemoveResult};

pub use iter::{Iter, IterMut, MergeIter, OwnedIter};

#[cfg(any(test, feature = "test"))]
pub mod tests;

pub struct PalmTree<K, V> {
    size: usize,
    root: Option<Box<Branch<K, V>>>,
}

impl<K, V> Default for PalmTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> PalmTree<K, V> {
    pub fn new() -> Self {
        Self {
            size: 0,
            root: None,
        }
    }
}

impl<K, V> PalmTree<K, V>
where
    K: Clone + Ord,
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
        fn push_stack<K: Clone, V>(child: Box<Branch<K, V>>, stack: &mut Vec<Box<Branch<K, V>>>) {
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
        let mut stack: Vec<Box<Branch<K, V>>> = Vec::new();
        let mut parent: Box<Branch<K, V>> = Box::new(Branch::new(1));
        let mut leaf: Box<Leaf<K, V>> = Box::new(Leaf::new());

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

    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(self, ..)
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut::new(self, ..)
    }

    pub fn range<R>(&self, range: R) -> Iter<'_, K, V>
    where
        R: RangeBounds<K>,
    {
        Iter::new(self, range)
    }

    pub fn range_mut<R>(&mut self, range: R) -> IterMut<'_, K, V>
    where
        R: RangeBounds<K>,
    {
        IterMut::new(self, range)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let len = self.size;
        if let Some(ref mut root) = self.root {
            // Special case: if a tree has size 0 but there is a root, it's because
            // we removed the last entry and the root has been left allocated.
            // Tree walking algos assume the tree has no empty nodes, so we have to
            // handle this as a special case.
            if len == 0 {
                // Make sure the delete trimmed the tree properly.
                debug_assert_eq!(0, root.len());
                debug_assert_eq!(1, root.height());

                root.push_key(key.clone());
                root.push_leaf(Box::new(Leaf::unit(key, value)));
                self.size = 1;
                return None;
            }
            match root.insert(key, value) {
                InsertResult::Added => {
                    self.size += 1;
                    None
                }
                InsertResult::Replaced(value) => Some(value),
                InsertResult::Full(key, value) => {
                    let height = root.height() + 1;
                    let key2 = root.last_key().unwrap().clone();
                    let child = std::mem::replace(&mut *root, Box::new(Branch::new(height)));
                    root.push_key(key2);
                    root.push_branch(child);
                    self.insert(key, value)
                }
            }
        } else {
            self.root = Some(Box::new(Branch::unit(Box::new(Leaf::unit(key, value)))));
            self.size = 1;
            None
        }
    }

    fn remove_result(&mut self, result: RemoveResult<K, V>) -> Option<(K, V)> {
        match result {
            RemoveResult::Deleted(key, value) => {
                self.size -= 1;
                Some((key, value))
            }
            // Deallocating the root if the tree becomes empty would be memory efficient,
            // but it would not be performance efficient, so we trim it and leave it.
            RemoveResult::DeletedAndEmpty(key, value) => {
                self.size -= 1;
                debug_assert_eq!(0, self.size); // We shouldn't be here if the tree isn't now empty.
                self.trim_root();
                Some((key, value))
            }
            RemoveResult::NotHere => None,
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<(K, V)> {
        let result = self.root.as_mut()?.remove(key);
        self.remove_result(result)
    }

    pub fn remove_lowest(&mut self) -> Option<(K, V)> {
        let result = self.root.as_mut()?.remove_lowest();
        self.remove_result(result)
    }

    pub fn remove_highest(&mut self) -> Option<(K, V)> {
        let result = self.root.as_mut()?.remove_highest();
        self.remove_result(result)
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
impl<K, V> Debug for PalmTree<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match &self.root {
            None => write!(f, "EmptyTree"),
            Some(root) => root.fmt(f),
        }
    }
}

#[cfg(not(feature = "tree_debug"))]
impl<K, V> Debug for PalmTree<K, V>
where
    K: Clone + Ord + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K, V> Clone for PalmTree<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            size: self.size,
        }
    }
}

impl<K, V> FromIterator<(K, V)> for PalmTree<K, V>
where
    K: Ord + Clone,
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

impl<'a, K, V> Index<&'a K> for PalmTree<K, V>
where
    K: Ord + Clone,
{
    type Output = V;

    fn index(&self, index: &K) -> &Self::Output {
        self.get(index).expect("no entry found for key")
    }
}

impl<'a, K, V> IndexMut<&'a K> for PalmTree<K, V>
where
    K: Ord + Clone,
{
    fn index_mut(&mut self, index: &K) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for key")
    }
}

impl<K, V> PartialEq for PalmTree<K, V>
where
    K: Ord + Clone,
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<K, V> Eq for PalmTree<K, V>
where
    K: Ord + Clone,
    V: Eq,
{
}

impl<K, V> PartialOrd for PalmTree<K, V>
where
    K: Ord + Clone,
    V: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<K, V> Ord for PalmTree<K, V>
where
    K: Ord + Clone,
    V: Ord,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<K, V> Extend<(K, V)> for PalmTree<K, V>
where
    K: Ord + Clone,
{
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<'a, K, V> Extend<(&'a K, &'a V)> for PalmTree<K, V>
where
    K: 'a + Ord + Copy,
    V: 'a + Copy,
{
    fn extend<I: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k.clone(), v.clone());
        }
    }
}

impl<K, V> Add for PalmTree<K, V>
where
    K: Ord + Clone,
{
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self::merge_right(self, other)
    }
}

impl<K, V> AddAssign for PalmTree<K, V>
where
    K: Ord + Clone,
{
    fn add_assign(&mut self, other: Self) {
        self.append_right(other)
    }
}

impl<'a, K, V> Add<&'a PalmTree<K, V>> for PalmTree<K, V>
where
    K: Ord + Copy,
    V: Copy,
{
    type Output = Self;

    fn add(self, other: &Self) -> Self::Output {
        Self::load(Self::merge_right_from(
            self.into_iter(),
            other.iter().map(|(k, v)| (k.clone(), v.clone())),
        ))
    }
}

impl<'a, K, V> AddAssign<&'a PalmTree<K, V>> for PalmTree<K, V>
where
    K: Ord + Copy,
    V: Copy,
{
    fn add_assign(&mut self, other: &'a PalmTree<K, V>) {
        let root = self.root.take();
        if root.is_none() {
            *self = other.clone();
        } else {
            *self = Self::load(Self::merge_right_from(
                OwnedIter::new(root, self.size),
                other.iter().map(|(k, v)| (k.clone(), v.clone())),
            ))
        }
    }
}

impl<K, V> Hash for PalmTree<K, V>
where
    K: Ord + Clone + Hash,
    V: Hash,
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

impl<'a, K, V> IntoIterator for &'a PalmTree<K, V>
where
    K: Ord + Clone,
{
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut PalmTree<K, V>
where
    K: Ord + Clone,
{
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V> IntoIterator for PalmTree<K, V>
where
    K: Ord + Clone,
{
    type Item = (K, V);
    type IntoIter = OwnedIter<K, V>;
    fn into_iter(self) -> Self::IntoIter {
        OwnedIter::new(self.root, self.size)
    }
}

impl<K, V> From<BTreeMap<K, V>> for PalmTree<K, V>
where
    K: Ord + Clone,
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
        let tree: PalmTree<usize, usize> = PalmTree::new();
        assert_eq!(None, tree.get(&1337));
    }

    #[test]
    fn lookup_single() {
        let mut tree: PalmTree<usize, usize> = PalmTree::new();
        tree.insert(1337, 31337);
        assert_eq!(None, tree.get(&1336));
        assert_eq!(Some(&31337), tree.get(&1337));
        assert_eq!(None, tree.get(&1338));
    }

    #[test]
    fn insert_in_sequence() {
        let mut tree: PalmTree<usize, usize> = PalmTree::new();
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
        let tree: PalmTree<usize, usize> = PalmTree::load((0..size).map(|i| (i, i)));
        // println!("{:?}", tree);
        for i in 0..size {
            assert_eq!(Some(&i), tree.get(&i));
        }
    }

    #[test]
    fn delete_delete_delete() {
        let mut tree: PalmTree<usize, usize> = PalmTree::load((0..131_072).map(|i| (i, i)));
        for i in 31337..41337 {
            assert_eq!(Some((i, i)), tree.remove(&i));
            assert_eq!(None, tree.remove(&i));
        }
    }

    #[test]
    fn insert_into_emptied_tree() {
        let mut tree: PalmTree<u8, u8> = PalmTree::new();
        tree.insert(0, 0);
        tree.remove(&0);
        tree.insert(0, 0);
        tree.insert(10, 10);

        let result: Vec<(u8, u8)> = tree.iter().map(|(k, v)| (*k, *v)).collect();
        let expected: Vec<(u8, u8)> = vec![(0, 0), (10, 10)];
        assert_eq!(expected, result);
    }
}
