// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#![forbid(rust_2018_idioms)]
#![deny(nonstandard_style)]
#![warn(
    unreachable_pub,
    // missing_debug_implementations,
    // missing_docs,
    // missing_doc_code_examples
)]
#![cfg_attr(core_intrinsics, feature(core_intrinsics))]

use std::fmt::{Debug, Error, Formatter};
use std::{iter::FromIterator, ops::RangeBounds};

pub mod asmtest;

mod arch;
mod search;
mod types;

mod leaf;
use leaf::Leaf;

mod branch;
use branch::Branch;

mod iter;
use iter::PalmTreeIter;
use types::{InsertResult, RemoveResult};

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
    /// will be validated.
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

    pub fn iter(&self) -> PalmTreeIter<'_, K, V> {
        PalmTreeIter::new(self, ..)
    }

    pub fn range<R>(&self, range: R) -> PalmTreeIter<'_, K, V>
    where
        R: RangeBounds<K>,
    {
        PalmTreeIter::new(self, range)
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
            self.root = Some(Box::new(Branch::unit(1, Box::new(Leaf::unit(key, value)))));
            self.size = 1;
            None
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<(K, V)> {
        if let Some(ref mut root) = self.root {
            match root.remove(key) {
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
        } else {
            None
        }
    }

    fn trim_root(&mut self) {
        if let Some(ref mut root) = self.root {
            // If a branch bearing root only has one child, we can replace the root with that child.
            while root.height() > 1 && root.len() == 1 {
                *root = root.remove_last_branch();
            }
        }
    }
}

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
