use crate::{
    branch::Branch,
    leaf::Leaf,
    search::{find_key_or_next, find_key_or_prev},
    PalmTree,
};
use std::{
    cmp::Ordering,
    ops::{Bound, RangeBounds},
};

/// Find the path to the leaf which contains `key` or the closest higher key.
fn path_for<'a, K, V>(
    tree: &'a Branch<K, V>,
    key: &K,
) -> Option<(Vec<(&'a Branch<K, V>, isize)>, &'a Leaf<K, V>)>
where
    K: Clone + Ord,
{
    let mut path = Vec::new();
    if let Some(leaf) = tree.leaf_for(key, Some(&mut path)) {
        Some((path, leaf))
    } else {
        None
    }
}

/// Step a stack forward by one entry.
///
/// If it returns `false`, you tried to step past the last entry. The stack
/// will be in an inconsistent state at this point and you should either panic
/// or discard it.
fn step_forward<'a, K, V>(
    stack: &mut Vec<(&'a Branch<K, V>, isize)>,
    leaf_ref: &mut Option<&'a Leaf<K, V>>,
    index_ref: &mut usize,
) -> bool {
    if let Some(leaf) = leaf_ref {
        *index_ref += 1;
        if *index_ref >= leaf.keys.len() {
            loop {
                // Pop a branch off the top of the stack and examine it.
                if let Some((branch, mut index)) = stack.pop() {
                    index += 1;
                    if index < branch.len() as isize {
                        // If we're not at the end yet, push the branch back on the stack and look at the next child.
                        stack.push((branch, index));
                        if branch.has_branches() {
                            // If it's a branch, push it on the stack and go through the loop again with this branch.
                            stack.push((branch.get_branch(index as usize), -1));
                            continue;
                        } else {
                            // If it's a leaf, this is our new leaf, we're done.
                            *leaf_ref = Some(branch.get_leaf(index as usize));
                            *index_ref = 0;
                            break;
                        }
                    } else {
                        // If this branch is exhausted, go round the loop again to look at its parent.
                        continue;
                    }
                } else {
                    return false;
                }
            }
        }
    }
    true
}

/// Step a stack back by one entry.
///
/// See notes for `step_forward`.
fn step_back<'a, K, V>(
    stack: &mut Vec<(&'a Branch<K, V>, isize)>,
    leaf_ref: &mut Option<&'a Leaf<K, V>>,
    index_ref: &mut usize,
) -> bool {
    if leaf_ref.is_some() {
        if *index_ref > 0 {
            *index_ref -= 1;
        } else {
            loop {
                // Pop a branch off the top of the stack and examine it.
                if let Some((branch, mut index)) = stack.pop() {
                    if index > 0 {
                        index -= 1;
                        // If we're not at the end yet, push the branch back on the stack and look at the next child.
                        stack.push((branch, index));
                        if branch.has_branches() {
                            let child = branch.get_branch(index as usize);
                            // If it's a branch, push it on the stack and go through the loop again with this branch.
                            stack.push((child, child.len() as isize));
                            continue;
                        } else {
                            let leaf = branch.get_leaf(index as usize);
                            // If it's a leaf, this is our new leaf, we're done.
                            *leaf_ref = Some(leaf);
                            *index_ref = leaf.keys.len() - 1;
                            break;
                        }
                    } else {
                        // If this branch is exhausted, go round the loop again to look at its parent.
                        continue;
                    }
                } else {
                    return false;
                }
            }
        }
    }
    true
}

pub struct PalmTreeIter<'a, K, V> {
    left_stack: Vec<(&'a Branch<K, V>, isize)>,
    left_leaf: Option<&'a Leaf<K, V>>,
    left_index: usize,

    right_stack: Vec<(&'a Branch<K, V>, isize)>,
    right_leaf: Option<&'a Leaf<K, V>>,
    right_index: usize,
}

impl<'a, K, V> PalmTreeIter<'a, K, V>
where
    K: Clone + Ord,
{
    pub(crate) fn new<R>(tree: &'a PalmTree<K, V>, range: R) -> Self
    where
        R: RangeBounds<K>,
    {
        match (range.start_bound(), range.end_bound()) {
            (Bound::Excluded(left), Bound::Excluded(right)) if left == right => {
                panic!("PalmTreeIter: start and end bounds are equal and excluding each other")
            }
            (Bound::Included(left), Bound::Included(right))
            | (Bound::Included(left), Bound::Excluded(right))
            | (Bound::Excluded(left), Bound::Included(right))
            | (Bound::Excluded(left), Bound::Excluded(right))
                if left.cmp(right) == Ordering::Greater =>
            {
                panic!("PalmTreeIter: range start is greater than range end");
            }
            _ => {}
        }

        if let Some(ref tree) = tree.root {
            let mut left_stack;
            let mut left_leaf;
            let mut left_index;
            match range.start_bound() {
                Bound::Included(key) => {
                    if let Some((path, target_leaf)) = path_for(tree, key) {
                        left_stack = path;
                        left_index = find_key_or_next(&target_leaf.keys, key);
                        left_leaf = Some(target_leaf);
                    } else {
                        // No target node for start bound, so it must be larger than the largest key; that's an empty iter.
                        left_stack = Vec::new();
                        left_index = 0;
                        left_leaf = None;
                    }
                }
                Bound::Excluded(key) => {
                    if let Some((path, target_leaf)) = path_for(tree, key) {
                        left_stack = path;
                        left_index = find_key_or_next(&target_leaf.keys, key);
                        left_leaf = Some(target_leaf);
                        if &target_leaf.keys[left_index] == key {
                            if !step_forward(&mut left_stack, &mut left_leaf, &mut left_index) {
                                // If we can't step forward, we were at the highest key already, so the iterator is empty.
                                left_stack = Vec::new();
                                left_index = 0;
                                left_leaf = None;
                            }
                        }
                    } else {
                        // No target node for start bound, so it must be larger than the largest key; that's an empty iter.
                        left_stack = Vec::new();
                        left_index = 0;
                        left_leaf = None;
                    }
                }
                Bound::Unbounded => {
                    let (stack, leaf, index) = tree.start_path();
                    left_stack = stack;
                    left_leaf = leaf;
                    left_index = index;
                }
            }

            let mut right_stack;
            let mut right_leaf;
            let mut right_index;
            match range.end_bound() {
                Bound::Included(key) => {
                    if let Some((path, target_leaf)) = path_for(tree, key) {
                        right_stack = path;
                        right_index = find_key_or_prev(&target_leaf.keys, key);
                        right_leaf = Some(target_leaf);
                    } else {
                        // No target node for end bound, so it must be larger than the largest key; get the path to that.
                        let (stack, leaf, index) = tree.end_path();
                        right_stack = stack;
                        right_leaf = leaf;
                        right_index = index;
                    }
                }
                Bound::Excluded(key) => {
                    if let Some((path, target_leaf)) = path_for(tree, key) {
                        right_stack = path;
                        right_index = find_key_or_prev(&target_leaf.keys, key);
                        right_leaf = Some(target_leaf);
                        if &target_leaf.keys[right_index] == key {
                            if !step_back(&mut right_stack, &mut right_leaf, &mut right_index) {
                                // If we can't step back, we were at the lowest key already, so the iterator is empty.
                                right_stack = Vec::new();
                                right_index = 0;
                                right_leaf = None;
                            }
                        } else if &target_leaf.keys[right_index] > key {
                            right_stack = Vec::new();
                            right_index = 0;
                            right_leaf = None;
                        }
                    } else {
                        // No target node for end bound, so it must be larger than the largest key; get the path to that.
                        let (stack, leaf, index) = tree.end_path();
                        right_stack = stack;
                        right_leaf = leaf;
                        right_index = index;
                    }
                }
                Bound::Unbounded => {
                    let (stack, leaf, index) = tree.end_path();
                    right_stack = stack;
                    right_leaf = leaf;
                    right_index = index;
                }
            }

            Self {
                left_stack,
                left_leaf,
                left_index,
                right_stack,
                right_leaf,
                right_index,
            }
        } else {
            // Tree has no root, iterator is empty.
            Self {
                left_stack: Vec::new(),
                left_leaf: None,
                left_index: 0,
                right_stack: Vec::new(),
                right_leaf: None,
                right_index: 0,
            }
        }
    }

    fn left_key(&self) -> Option<&'a K> {
        self.left_leaf.map(|leaf| &leaf.keys[self.left_index])
    }

    fn right_key(&self) -> Option<&'a K> {
        self.right_leaf.map(|leaf| &leaf.keys[self.right_index])
    }

    fn step_forward(&mut self) {
        let result = step_forward(
            &mut self.left_stack,
            &mut self.left_leaf,
            &mut self.left_index,
        );
        debug_assert!(result);
    }

    fn step_back(&mut self) {
        let result = step_back(
            &mut self.right_stack,
            &mut self.right_leaf,
            &mut self.right_index,
        );
        debug_assert!(result);
    }
}

impl<'a, K, V> Iterator for PalmTreeIter<'a, K, V>
where
    K: Clone + Ord,
{
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let left_key = self.left_key()?;
        let right_key = self.right_key()?;
        // If left key is greather than right key, we're done.
        let cmp = left_key.cmp(right_key);
        if cmp == Ordering::Greater {
            self.left_leaf = None;
            self.right_leaf = None;
            return None;
        }
        let value = &self.left_leaf.unwrap().values[self.left_index];
        if cmp == Ordering::Equal {
            self.left_leaf = None;
            self.right_leaf = None;
        } else {
            self.step_forward();
        }
        Some((left_key, value))
    }
}

impl<'a, K, V> DoubleEndedIterator for PalmTreeIter<'a, K, V>
where
    K: Clone + Ord,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let left_key = self.left_key()?;
        let right_key = self.right_key()?;
        // If left key is greather than right key, we're done.
        let cmp = left_key.cmp(right_key);
        if cmp == Ordering::Greater {
            self.left_leaf = None;
            self.right_leaf = None;
            return None;
        }
        let value = &self.right_leaf.unwrap().values[self.right_index];
        if cmp == Ordering::Equal {
            self.left_leaf = None;
            self.right_leaf = None;
        } else {
            self.step_back();
        }
        Some((right_key, value))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn iterate_single_leaf() {
        let size = 64usize;
        let tree = PalmTree::load((0..size).map(|i| (i, i)));
        // let expected: Vec<_> = (0..size).map(|i| (i, i)).collect();
        // let result: Vec<_> = tree.iter().map(|(k, v)| (*k, *v)).collect();
        tree.iter().for_each(|i| {
            criterion::black_box(i);
        });
        // assert_eq!(expected, result);
    }

    #[test]
    fn iterate_forward() {
        let size = 65536usize;
        let tree = PalmTree::load((0..size).map(|i| (i, i)));
        let expected: Vec<_> = (0..size).map(|i| (i, i)).collect();
        let result: Vec<_> = tree.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn iterate_backward() {
        let size = 65536usize;
        let tree = PalmTree::load((0..size).map(|i| (i, i)));
        let expected: Vec<_> = (0..size).map(|i| (i, i)).rev().collect();
        let result: Vec<_> = tree.iter().map(|(k, v)| (*k, *v)).rev().collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn empty_range_iter() {
        let tree = PalmTree::load((0..1usize).map(|i| (i, i)));
        let expected = Vec::<(usize, usize)>::new();
        let result: Vec<_> = tree.range(0..0).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn wide_end_range_iter() {
        let tree = PalmTree::load((0..1usize).map(|i| (i, i)));
        let expected = vec![(0usize, 0usize)];
        let result: Vec<_> = tree.range(0..255).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn wide_start_range_iter() {
        let tree = PalmTree::load((0..1usize).map(|i| (i, i)));
        let expected: Vec<(usize, usize)> = vec![];
        let result: Vec<_> = tree.range(100..).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    #[should_panic]
    fn descending_range_iter() {
        let tree = PalmTree::load((0..1usize).map(|i| (i, i)));
        let expected = Vec::<(usize, usize)>::new();
        let result: Vec<_> = tree.range(255..0).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn end_before_first_key_iter() {
        let tree = PalmTree::load((1..2usize).map(|i| (i, i)));
        let expected: Vec<(usize, usize)> = vec![];
        let result: Vec<_> = tree.range(..0).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn start_after_last_key_iter() {
        let tree = PalmTree::load((1..2usize).map(|i| (i, i)));
        let expected: Vec<(usize, usize)> = vec![];
        let result: Vec<_> = tree.range(3..).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn end_before_last_key_iter() {
        let tree = PalmTree::load((0..2usize).map(|i| (i, i)));
        let expected: Vec<(usize, usize)> = vec![(0, 0)];
        let result: Vec<_> = tree.range(..=0).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }
}
