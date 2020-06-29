use crate::{arch::prefetch, branch::Branch, leaf::Leaf, types::MaxHeight};
use sized_chunks::Chunk;
use std::{
    fmt::{Debug, Error, Formatter},
    marker::PhantomData,
};

type PtrPath<K, V> = Chunk<(*const Branch<K, V>, isize), MaxHeight>;

/// Find 'key' in 'keys', or the closest higher value.
///
/// If every value in `keys` is lower than `key`, `None` will be returned.
///
/// This is a checked version of `find_key_or_next`. No assumption about
/// the content of `keys` is needed, and it will never panic.
pub(crate) fn find_key<K>(keys: &[K], key: &K) -> Option<usize>
where
    K: Ord,
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

pub(crate) fn find_key_linear<K>(keys: &[K], target: &K) -> Option<usize>
where
    K: Ord,
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
pub(crate) fn find_key_or_next<K>(keys: &[K], key: &K) -> usize
where
    K: Ord,
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
pub(crate) fn find_key_or_prev<K>(keys: &[K], key: &K) -> usize
where
    K: Ord,
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
pub(crate) struct PathedPointer<L, K, V> {
    stack: PtrPath<K, V>,
    leaf: *const Leaf<K, V>,
    index: usize,
    lifetime: PhantomData<L>,
}

impl<L, K, V> Clone for PathedPointer<L, K, V> {
    fn clone(&self) -> Self {
        Self {
            stack: self.stack.clone(),
            leaf: self.leaf.clone(),
            index: self.index,
            lifetime: PhantomData,
        }
    }
}

fn walk_path<'a, K, V>(
    mut branch: &'a Branch<K, V>,
    key: &K,
    path: &mut PtrPath<K, V>,
) -> Option<&'a Leaf<K, V>>
where
    K: Clone + Ord,
{
    loop {
        if let Some(index) = find_key(branch.keys(), key) {
            path.push_back((branch, index as isize));
            if branch.height() > 1 {
                branch = branch.get_branch(index);
            } else {
                return Some(branch.get_leaf(index));
            }
        } else {
            return None;
        }
    }
}

/// Find the path to the leaf which contains `key` or the closest higher key.
fn path_for<'a, K, V>(tree: &'a Branch<K, V>, key: &K) -> Option<(PtrPath<K, V>, &'a Leaf<K, V>)>
where
    K: Clone + Ord,
{
    let mut path: PtrPath<K, V> = Chunk::new();
    walk_path(tree, key, &mut path).map(|leaf| (path, leaf))
}

impl<L, K, V> PathedPointer<L, K, V>
where
    K: Clone + Ord,
{
    pub(crate) fn null() -> Self {
        Self {
            stack: Chunk::new(),
            leaf: std::ptr::null(),
            index: 0,
            lifetime: PhantomData,
        }
    }

    /// Find `key` and return `Ok(path)` for a key match or `Err(path)` for an absent key with
    /// the path to the leaf it should be in. This path will be null if the key is larger than
    /// the tree's current highest key.
    pub(crate) fn exact_key(tree: &Branch<K, V>, key: &K) -> Result<Self, Self> {
        if let Some((stack, leaf)) = path_for(tree, key) {
            match leaf.keys().binary_search(key) {
                Ok(index) => Ok(Self {
                    stack,
                    leaf,
                    index,
                    lifetime: PhantomData,
                }),
                Err(index) => Err(Self {
                    stack,
                    leaf,
                    index,
                    lifetime: PhantomData,
                }),
            }
        } else {
            Err(Self::null())
        }
    }

    /// Find `key` or the first higher key.
    pub(crate) fn key_or_higher(tree: &Branch<K, V>, key: &K) -> Self {
        let mut ptr = Self::null();
        if let Some((path, leaf)) = path_for(tree, key) {
            ptr.stack = path;
            ptr.index = find_key_or_next(leaf.keys(), key);
            ptr.leaf = leaf;
            // find_key_or_next assumes the highest key in the leaf isn't lower than `key`, but a search
            // through a tree with branch keys higher than the highest key present in the leaf can take
            // you to a node where this doesn't hold, so we have to check if we need to step forward.
            // If we do, we can depend on the next neighbour node containing the right key as its first
            // entry.
            unsafe {
                if ptr.key().unwrap() < key && !ptr.step_forward() {
                    // If we can't step forward, we were at the highest key already, so the iterator is empty.
                    ptr = Self::null();
                }
            }
        } else {
            // No target node for start bound means the key is higher than our highest value, so we leave ptr empty.
        }
        ptr
    }

    /// Find the first key higher than `key`.
    pub(crate) fn higher_than_key(tree: &Branch<K, V>, key: &K) -> Self {
        let mut ptr = Self::null();
        if let Some((path, leaf)) = path_for(tree, key) {
            ptr.stack = path;
            ptr.index = find_key_or_next(leaf.keys(), key);
            ptr.leaf = leaf;
            unsafe {
                if &leaf.keys()[ptr.index] == key && !ptr.step_forward() {
                    // If we can't step forward, we were at the highest key already, so the iterator is empty.
                    return Self::null();
                }
            }
        } else {
            // No target node for start bound means the key is higher than our highest value, so we leave ptr empty.
        }
        ptr
    }

    /// Find `key` or the first lower key.
    pub(crate) fn key_or_lower(tree: &Branch<K, V>, key: &K) -> Self {
        if let Some((path, leaf)) = path_for(tree, key) {
            let mut ptr = Self::null();
            ptr.stack = path;
            ptr.index = find_key_or_next(leaf.keys(), key);
            ptr.leaf = leaf;
            ptr
        } else {
            // No target node for end bound means it's past the largest key, so get a path to the end of the tree.
            Self::highest(tree)
        }
    }

    /// Find the first key lower than `key`.
    pub(crate) fn lower_than_key(tree: &Branch<K, V>, key: &K) -> Self {
        if let Some((path, leaf)) = path_for(tree, key) {
            let mut ptr = Self::null();
            ptr.stack = path;
            ptr.index = find_key_or_prev(leaf.keys(), key);
            ptr.leaf = leaf;
            // If we've found a value equal to key, we step back one key.
            // If we've found a value higher than key, we're one branch ahead of the target key and step back.
            unsafe {
                if &leaf.keys()[ptr.index] >= key && !ptr.step_back() {
                    // If we can't step back, we were at the lowest key already, so the iterator is empty.
                    return Self::null();
                }
            }
            ptr
        } else {
            // No target node for end bound, so it must be larger than the largest key; get the path to that.
            Self::highest(tree)
        }
    }

    /// Find the lowest key in the tree.
    pub(crate) fn lowest(tree: &Branch<K, V>) -> Self {
        let mut branch = tree;
        let mut stack = PtrPath::new();
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
                    leaf: branch.get_leaf(0),
                    index: 0,
                    lifetime: PhantomData,
                };
            }
        }
    }

    /// Find the highest key in the tree.
    pub(crate) fn highest(tree: &Branch<K, V>) -> Self {
        let mut branch = tree;
        let mut stack = PtrPath::new();
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
                    leaf,
                    index: leaf.len() - 1,
                    lifetime: PhantomData,
                };
            }
        }
    }

    /// Step a pointer forward by one entry.
    ///
    /// If it returns `false`, you tried to step past the last entry.
    /// If this happens, the pointer is now a null pointer.
    pub(crate) unsafe fn step_forward(&mut self) -> bool {
        if !self.is_null() {
            self.index += 1;
            if self.index >= (*self.leaf).keys().len() {
                loop {
                    // Pop a branch off the top of the stack and examine it.
                    if !self.stack.is_empty() {
                        let (branch, mut index) = self.stack.pop_back();
                        index += 1;
                        if index < (*branch).len() as isize {
                            // If we're not at the end yet, push the branch back on the stack and look at the next child.
                            self.stack.push_back((branch, index));
                            if (*branch).has_branches() {
                                // If it's a branch, push it on the stack and go through the loop again with this branch.
                                self.stack
                                    .push_back(((*branch).get_branch(index as usize), -1));
                                continue;
                            } else {
                                // If it's a leaf, this is our new leaf, we're done.
                                self.leaf = (*branch).get_leaf(index as usize);
                                self.index = 0;
                                // Prefetch the next leaf.
                                let next_index = (index + 1) as usize;
                                if next_index < (*branch).len() {
                                    prefetch((*branch).get_leaf(next_index));
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
    pub(crate) unsafe fn step_back(&mut self) -> bool {
        if !self.is_null() {
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
                            if (*branch).has_branches() {
                                let child = (*branch).get_branch(index as usize);
                                // If it's a branch, push it on the stack and go through the loop again with this branch.
                                self.stack.push_back((child, child.len() as isize));
                                continue;
                            } else {
                                // If it's a leaf, this is our new leaf, we're done.
                                self.leaf = (*branch).get_leaf(index as usize);
                                self.index = (*self.leaf).keys().len() - 1;
                                // Prefetch the next leaf.
                                if index > 0 {
                                    prefetch((*branch).get_leaf(index as usize - 1));
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

    /// Remove the entry being pointed at.
    ///
    /// You're responsible for ensuring there is indeed an entry being pointed at.
    pub(crate) unsafe fn remove(mut self) -> (K, V) {
        // TODO need a strategy for rebalancing after remove
        let index = self.index;
        let leaf = self.deref_mut_leaf().unwrap();
        let key = leaf.keys.remove(index);
        let value = leaf.values.remove(index);
        if leaf.is_empty() {
            loop {
                let (branch, index) = self.stack.pop_back();
                let branch = &mut *(branch as *mut Branch<K, V>);
                let index = index as usize;
                branch.remove_key(index);
                if branch.has_leaves() {
                    branch.remove_leaf(index);
                } else {
                    branch.remove_branch(index);
                }
                if !branch.is_empty() || self.stack.is_empty() {
                    return (key, value);
                }
            }
        } else {
            (key, value)
        }
    }

    /// Insert a key at the index being pointed at.
    ///
    /// You're responsible for ensuring that something is being pointed at,
    /// that what's being pointed at is the location in the leaf where this
    /// key should be inserted, and that the key isn't already there.
    /// This is the assumption validated by the `exact_key` constructor when it
    /// returns a non-null `Err` value.
    pub(crate) unsafe fn insert(mut self, key: K, value: V) -> Result<Self, (K, V)> {
        let index = self.index;
        let leaf = self.deref_mut_leaf().unwrap();
        if !leaf.is_full() {
            leaf.keys.insert(index, key);
            leaf.values.insert(index, value);
            Ok(self)
        } else {
            // Walk up the tree to find somewhere to split.
            loop {
                if self.stack.is_empty() {
                    return Err((key, value));
                }
                let (branch, index) = self.stack.pop_back();
                let branch = &mut *(branch as *mut Branch<K, V>);
                let index = index as usize;
                if !branch.is_full() {
                    let choose_index = if branch.has_branches() {
                        let (left, right) = branch.remove_branch(index).split();
                        let left_highest = left.highest();
                        let choose_index = if &key <= left_highest {
                            index
                        } else {
                            index + 1
                        };
                        branch.insert_key(index, left_highest.clone());
                        branch.insert_branch_pair(index, left, right);
                        choose_index
                    } else {
                        let (left, right) = branch.remove_leaf(index).split();
                        let left_highest = left.highest();
                        let choose_index = if &key <= left_highest {
                            index
                        } else {
                            index + 1
                        };
                        branch.insert_key(index, left_highest.clone());
                        branch.insert_leaf_pair(index, left, right);
                        choose_index
                    };
                    // We're going to walk down either the left or the right hand branch of our split.
                    // We're guaranteed to find a leaf, but it might be full if we split a higher branch,
                    // so we might have to go back up and split further.
                    let leaf = if branch.has_branches() {
                        walk_path(branch.get_branch(choose_index), &key, &mut self.stack)
                    } else {
                        Some(branch.get_leaf(choose_index))
                    };
                    if let Some(leaf) = leaf {
                        if !leaf.is_full() {
                            let index = leaf
                                .keys
                                .binary_search(&key)
                                .expect_err("tried to insert() a key that already exists");
                            self.leaf = leaf;
                            self.index = index;
                            assert!(
                                index <= leaf.keys.len(),
                                "index {} > len {}",
                                index,
                                leaf.keys.len()
                            );
                            let leaf = self.deref_mut_leaf().unwrap();
                            leaf.keys.insert(index, key);
                            leaf.values.insert(index, value);
                            return Ok(self);
                        }
                    } else {
                        unreachable!("walk_path() failed to produce a leaf, even though the leaf should be there!")
                    }
                }
            }
        }
    }

    /// Insert a value at the right edge of the tree.
    /// If it returns false, you need to split the root and try again.
    ///
    /// This must only be called on a null pointer, and the key provided must
    /// be higher than the tree's current maximum.
    pub(crate) unsafe fn push_last(
        mut self,
        root: &mut Branch<K, V>,
        key: K,
        value: V,
    ) -> Result<Self, (K, V)> {
        let mut branch = root;
        let mut index;
        loop {
            index = branch.len() - 1;
            debug_assert!(branch.highest() < &key);
            branch.keys[index] = key.clone();
            self.stack.push_back((branch, index as isize));
            if branch.has_branches() {
                branch = branch.get_branch_mut(index);
            } else {
                break;
            }
        }
        self.leaf = branch.get_leaf(index);
        self.index = (*self.leaf).len();
        self.insert(key, value)
    }

    pub(crate) fn clear(&mut self) {
        self.leaf = std::ptr::null();
    }

    pub(crate) fn is_null(&self) -> bool {
        self.leaf.is_null()
    }

    pub(crate) unsafe fn deref_leaf<'a>(&'a self) -> Option<&'a Leaf<K, V>> {
        self.leaf.as_ref()
    }

    pub(crate) unsafe fn deref_mut_leaf<'a>(&'a mut self) -> Option<&'a mut Leaf<K, V>> {
        (self.leaf as *mut Leaf<K, V>).as_mut()
    }

    pub(crate) unsafe fn into_entry_mut<'a>(self) -> (&'a mut K, &'a mut V)
    where
        L: 'a,
    {
        let index = self.index;
        let leaf = &mut *(self.leaf as *mut Leaf<K, V>);
        let key: *mut K = &mut leaf.keys[index];
        let value: *mut V = &mut leaf.values[index];
        (&mut *key, &mut *value)
    }

    pub(crate) unsafe fn key<'a>(&'a self) -> Option<&'a K> {
        self.deref_leaf().map(|leaf| &leaf.keys[self.index])
    }

    pub(crate) unsafe fn value<'a>(&'a self) -> Option<&'a V> {
        self.deref_leaf().map(|leaf| &leaf.values[self.index])
    }

    pub(crate) unsafe fn value_mut<'a>(&'a mut self) -> Option<&'a mut V> {
        let index = self.index;
        self.deref_mut_leaf().map(|leaf| &mut leaf.values[index])
    }
}

impl<L, K, V> Debug for PathedPointer<L, K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "PathedPointer")
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
