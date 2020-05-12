use crate::{arch::prefetch, branch::Branch, leaf::Leaf, types::Path};
use sized_chunks::{types::ChunkLength, Chunk};

/// Find 'key' in 'keys', or the closest higher value.
///
/// If every value in `keys` is lower than `key`, `None` will be returned.
///
/// This is a checked version of `find_key_or_next`. No assumption about
/// the content of `keys` is needed, and it will never panic.
pub(crate) fn find_key<K, S>(keys: &Chunk<K, S>, key: &K) -> Option<usize>
where
    K: Ord,
    S: ChunkLength<K>,
{
    let size = keys.len();
    if size == 0 {
        return None;
    }

    let mut low = 0;
    let mut high = size - 1;
    while low != high {
        let mid = (low + high) / 2;
        if &keys[mid] < key {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    if low == size || &keys[low] < key {
        None
    } else {
        Some(low)
    }
}

pub(crate) fn find_key_linear<K, S>(keys: &Chunk<K, S>, target: &K) -> Option<usize>
where
    K: Ord,
    S: ChunkLength<K>,
{
    for (index, key) in keys.iter().enumerate() {
        if target <= key {
            return Some(index);
        }
    }
    None
}

/// Find `key` in `keys`, or the closest higher value.
///
/// This function assumes the highest value in `keys` is
/// not lower than `key`, and that `keys` is not empty.
///
/// If `key` is higher than the highest value in `keys`, the
/// index of the highest value will be returned.
///
/// If `keys` is empty, this function will panic.
pub(crate) fn find_key_or_next<K, S>(keys: &Chunk<K, S>, key: &K) -> usize
where
    K: Ord,
    S: ChunkLength<K>,
{
    let size = keys.len();
    let mut low = 0;
    let mut high = size - 1;
    while low != high {
        let mid = (low + high) / 2;
        if &keys[mid] < key {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

/// Find `key` in `keys`, or the closest lower value.
///
/// Invariants as in `find_or_next` above apply, but reversed.
pub(crate) fn find_key_or_prev<K, S>(keys: &Chunk<K, S>, key: &K) -> usize
where
    K: Ord,
    S: ChunkLength<K>,
{
    let size = keys.len();
    let mut low = 0;
    let mut high = size - 1;
    while low != high {
        let mid = (low + high + 1) / 2;
        if &keys[mid] > key {
            high = mid - 1;
        } else {
            low = mid;
        }
    }
    low
}

/// A pointer to a leaf entry which can be stepped forwards and backwards.
pub(crate) struct PathedPointer<'a, K, V> {
    stack: Path<'a, K, V>,
    leaf: Option<&'a Leaf<K, V>>,
    index: usize,
}

/// Find the path to the leaf which contains `key` or the closest higher key.
fn path_for<'a, K, V>(tree: &'a Branch<K, V>, key: &K) -> Option<(Path<'a, K, V>, &'a Leaf<K, V>)>
where
    K: Clone + Ord,
{
    let mut path = Path::new();
    if let Some(leaf) = tree.leaf_for(key, Some(&mut path)) {
        Some((path, leaf))
    } else {
        None
    }
}

impl<'a, K, V> PathedPointer<'a, K, V>
where
    K: Clone + Ord,
{
    pub(crate) fn null() -> Self {
        Self {
            stack: Path::new(),
            leaf: None,
            index: 0,
        }
    }

    /// Find `key` or the first higher key.
    pub(crate) fn key_or_higher(tree: &'a Branch<K, V>, key: &K) -> Self {
        let mut ptr = Self::null();
        if let Some((path, leaf)) = path_for(tree, key) {
            ptr.stack = path;
            ptr.index = find_key_or_next(&leaf.keys, key);
            ptr.leaf = Some(leaf);
            // find_key_or_next assumes the highest key in the leaf isn't lower than `key`, but a search
            // through a tree with branch keys higher than the highest key present in the leaf can take
            // you to a node where this doesn't hold, so we have to check if we need to step forward.
            // If we do, we can depend on the next neighbour node containing the right key as its first
            // entry.
            if ptr.key().unwrap() < key && !ptr.step_forward() {
                // If we can't step forward, we were at the highest key already, so the iterator is empty.
                ptr = Self::null();
            }
        } else {
            // No target node for start bound means the key is higher than our highest value, so we leave ptr empty.
        }
        ptr
    }

    /// Find the first key higher than `key`.
    pub(crate) fn higher_than_key(tree: &'a Branch<K, V>, key: &K) -> Self {
        let mut ptr = Self::null();
        if let Some((path, leaf)) = path_for(tree, key) {
            ptr.stack = path;
            ptr.index = find_key_or_next(&leaf.keys, key);
            ptr.leaf = Some(leaf);
            if &leaf.keys[ptr.index] == key && !ptr.step_forward() {
                // If we can't step forward, we were at the highest key already, so the iterator is empty.
                ptr = Self::null();
            }
        } else {
            // No target node for start bound means the key is higher than our highest value, so we leave ptr empty.
        }
        ptr
    }

    /// Find `key` or the first lower key.
    pub(crate) fn key_or_lower(tree: &'a Branch<K, V>, key: &K) -> Self {
        if let Some((path, leaf)) = path_for(tree, key) {
            let mut ptr = Self::null();
            ptr.stack = path;
            ptr.index = find_key_or_next(&leaf.keys, key);
            ptr.leaf = Some(leaf);
            ptr
        } else {
            // No target node for end bound means it's past the largest key, so get a path to the end of the tree.
            Self::highest(tree)
        }
    }

    /// Find the first key lower than `key`.
    pub(crate) fn lower_than_key(tree: &'a Branch<K, V>, key: &K) -> Self {
        if let Some((path, leaf)) = path_for(tree, key) {
            let mut ptr = Self::null();
            ptr.stack = path;
            ptr.index = find_key_or_prev(&leaf.keys, key);
            ptr.leaf = Some(leaf);
            // If we've found a value equal to key, we step back one key.
            // If we've found a value higher than key, we're one branch ahead of the target key and step back.
            if &leaf.keys[ptr.index] >= key && !ptr.step_back() {
                // If we can't step back, we were at the lowest key already, so the iterator is empty.
                return Self::null();
            }
            ptr
        } else {
            // No target node for end bound, so it must be larger than the largest key; get the path to that.
            Self::highest(tree)
        }
    }

    /// Find the lowest key in the tree.
    pub(crate) fn lowest(tree: &'a Branch<K, V>) -> Self {
        let mut branch = tree;
        let mut stack = Path::new();
        loop {
            if branch.is_empty() {
                return Self::null();
            }
            stack.push_back((branch, 0));
            if branch.has_branches() {
                branch = branch.get_branch(0);
            } else {
                return Self {
                    stack,
                    leaf: Some(branch.get_leaf(0)),
                    index: 0,
                };
            }
        }
    }

    /// Find the highest key in the tree.
    pub(crate) fn highest(tree: &'a Branch<K, V>) -> Self {
        let mut branch = tree;
        let mut stack = Path::new();
        loop {
            if branch.is_empty() {
                return Self::null();
            }
            let index = branch.len() - 1;
            stack.push_back((branch, index as isize));
            if branch.has_branches() {
                branch = branch.get_branch(index);
            } else {
                let leaf = branch.get_leaf(index);
                return Self {
                    stack,
                    leaf: Some(leaf),
                    index: leaf.len() - 1,
                };
            }
        }
    }

    /// Step a pointer forward by one entry.
    ///
    /// If it returns `false`, you tried to step past the last entry.
    /// If this happens, the pointer is now a null pointer.
    pub(crate) fn step_forward(&mut self) -> bool {
        if let Some(leaf) = self.leaf {
            self.index += 1;
            if self.index >= leaf.keys.len() {
                loop {
                    // Pop a branch off the top of the stack and examine it.
                    if !self.stack.is_empty() {
                        let (branch, mut index) = self.stack.pop_back();
                        index += 1;
                        if index < branch.len() as isize {
                            // If we're not at the end yet, push the branch back on the stack and look at the next child.
                            self.stack.push_back((branch, index));
                            if branch.has_branches() {
                                // If it's a branch, push it on the stack and go through the loop again with this branch.
                                self.stack
                                    .push_back((branch.get_branch(index as usize), -1));
                                continue;
                            } else {
                                // If it's a leaf, this is our new leaf, we're done.
                                self.leaf = Some(branch.get_leaf(index as usize));
                                self.index = 0;
                                // Prefetch the next leaf.
                                let next_index = (index + 1) as usize;
                                if next_index < branch.len() {
                                    unsafe { prefetch(branch.get_leaf(next_index)) };
                                }
                                break;
                            }
                        } else {
                            // If this branch is exhausted, go round the loop again to look at its parent.
                            continue;
                        }
                    } else {
                        self.clear();
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Step a pointer back by one entry.
    ///
    /// See notes for `step_forward`.
    pub(crate) fn step_back(&mut self) -> bool {
        if self.leaf.is_some() {
            if self.index > 0 {
                self.index -= 1;
            } else {
                loop {
                    // Pop a branch off the top of the stack and examine it.
                    if !self.stack.is_empty() {
                        let (branch, mut index) = self.stack.pop_back();
                        if index > 0 {
                            index -= 1;
                            // If we're not at the end yet, push the branch back on the stack and look at the next child.
                            self.stack.push_back((branch, index));
                            if branch.has_branches() {
                                let child = branch.get_branch(index as usize);
                                // If it's a branch, push it on the stack and go through the loop again with this branch.
                                self.stack.push_back((child, child.len() as isize));
                                continue;
                            } else {
                                let leaf = branch.get_leaf(index as usize);
                                // If it's a leaf, this is our new leaf, we're done.
                                self.leaf = Some(leaf);
                                self.index = leaf.keys.len() - 1;
                                // Prefetch the next leaf.
                                if index > 0 {
                                    unsafe { prefetch(branch.get_leaf(index as usize - 1)) };
                                }
                                break;
                            }
                        } else {
                            // If this branch is exhausted, go round the loop again to look at its parent.
                            continue;
                        }
                    } else {
                        self.clear();
                        return false;
                    }
                }
            }
        }
        true
    }

    pub(crate) fn clear(&mut self) {
        self.leaf = None;
    }

    pub(crate) fn is_null(&self) -> bool {
        self.leaf.is_none()
    }

    pub(crate) fn key(&self) -> Option<&'a K> {
        self.leaf.map(|leaf| &leaf.keys[self.index])
    }

    pub(crate) fn value(&self) -> Option<&'a V> {
        self.leaf.map(|leaf| &leaf.values[self.index])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn test_find_key() {
        let keys: Chunk<usize> = Chunk::from_iter(vec![2, 4, 6, 8]);
        assert_eq!(Some(0), find_key(&keys, &0));
        assert_eq!(Some(0), find_key(&keys, &1));
        assert_eq!(Some(0), find_key(&keys, &2));
        assert_eq!(Some(1), find_key(&keys, &3));
        assert_eq!(Some(1), find_key(&keys, &4));
        assert_eq!(Some(2), find_key(&keys, &5));
        assert_eq!(Some(2), find_key(&keys, &6));
        assert_eq!(Some(3), find_key(&keys, &7));
        assert_eq!(Some(3), find_key(&keys, &8));
        assert_eq!(None, find_key(&keys, &9));
        assert_eq!(None, find_key(&keys, &10));
        assert_eq!(None, find_key(&keys, &31337));
    }

    #[test]
    fn test_find_key_or_next() {
        let keys: Chunk<usize> = Chunk::from_iter(vec![2, 4, 6, 8]);
        assert_eq!(0, find_key_or_next(&keys, &0));
        assert_eq!(0, find_key_or_next(&keys, &1));
        assert_eq!(0, find_key_or_next(&keys, &2));
        assert_eq!(1, find_key_or_next(&keys, &3));
        assert_eq!(1, find_key_or_next(&keys, &4));
        assert_eq!(2, find_key_or_next(&keys, &5));
        assert_eq!(2, find_key_or_next(&keys, &6));
        assert_eq!(3, find_key_or_next(&keys, &7));
        assert_eq!(3, find_key_or_next(&keys, &8));
    }

    #[test]
    fn test_find_key_or_prev() {
        let keys: Chunk<usize> = Chunk::from_iter(vec![2, 4, 6, 8]);
        assert_eq!(0, find_key_or_prev(&keys, &2));
        assert_eq!(0, find_key_or_prev(&keys, &3));
        assert_eq!(1, find_key_or_prev(&keys, &4));
        assert_eq!(1, find_key_or_prev(&keys, &5));
        assert_eq!(2, find_key_or_prev(&keys, &6));
        assert_eq!(2, find_key_or_prev(&keys, &7));
        assert_eq!(3, find_key_or_prev(&keys, &8));
        assert_eq!(3, find_key_or_prev(&keys, &9));
        assert_eq!(3, find_key_or_prev(&keys, &10));
    }
}
