use crate::{
    leaf::Leaf,
    search::{find_key, find_key_linear},
    types::{InsertResult, MaxHeight, NodeSize, Path, PathMut, RemoveResult},
};
use node::Node;
use sized_chunks::Chunk;
use std::fmt::{Debug, Error, Formatter};
use typenum::Unsigned;

/// A branch node holds mappings of high keys to child nodes.
pub(crate) struct Branch<K, V> {
    height: usize,
    keys: Chunk<K, NodeSize>,
    children: Chunk<Node<K, V>, NodeSize>,
}

impl<K, V> Drop for Branch<K, V> {
    fn drop(&mut self) {
        while !self.children.is_empty() {
            // The `Node` type can't drop itself because it doesn't know
            // whether it's a Branch or a Leaf, so we *must* drop every `Node`
            // from the `Branch` it's stored in.
            let node = self.children.pop_front();
            if self.height > 1 {
                unsafe { node.unwrap_branch() };
            } else {
                unsafe { node.unwrap_leaf() };
            }
        }
    }
}

impl<K, V> Branch<K, V> {
    pub(crate) fn new(height: usize) -> Self {
        debug_assert!(height <= MaxHeight::USIZE);
        Branch {
            height,
            keys: Chunk::new(),
            children: Chunk::new(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.keys.is_full()
    }

    pub(crate) fn highest(&self) -> &K {
        self.keys.last().unwrap()
    }

    pub(crate) fn has_leaves(&self) -> bool {
        self.height == 1
    }

    pub(crate) fn has_branches(&self) -> bool {
        self.height > 1
    }

    pub(crate) fn height(&self) -> usize {
        self.height
    }

    pub(crate) fn get_branch(&self, index: usize) -> &Branch<K, V> {
        debug_assert!(self.has_branches()); // Only branches higher than 1 have Branch children.
        unsafe { self.children[index].as_branch() }
    }

    pub(crate) fn get_leaf(&self, index: usize) -> &Leaf<K, V> {
        debug_assert!(self.has_leaves()); // Only branches at height 1 have Leaf children.
        unsafe { self.children[index].as_leaf() }
    }

    pub(crate) fn get_branch_mut(&mut self, index: usize) -> &mut Branch<K, V> {
        debug_assert!(self.has_branches()); // Only branches higher than 1 have Branch children.
        unsafe { self.children[index].as_branch_mut() }
    }

    pub(crate) fn get_leaf_mut(&mut self, index: usize) -> &mut Leaf<K, V> {
        debug_assert!(self.has_leaves()); // Only branches at height 1 have Leaf children.
        unsafe { self.children[index].as_leaf_mut() }
    }

    pub(crate) fn last_key(&self) -> Option<&K> {
        self.keys.last()
    }

    pub(crate) fn push_key(&mut self, key: K) {
        self.keys.push_back(key)
    }

    pub(crate) fn push_branch(&mut self, branch: Box<Branch<K, V>>) {
        debug_assert!(self.has_branches());
        self.children.push_back(branch.into())
    }

    pub(crate) fn push_leaf(&mut self, leaf: Box<Leaf<K, V>>) {
        debug_assert!(self.has_leaves());
        self.children.push_back(leaf.into())
    }

    fn remove_branch(&mut self, index: usize) -> Box<Branch<K, V>> {
        debug_assert!(self.has_branches());
        unsafe { self.children.remove(index).unwrap_branch() }
    }

    fn remove_leaf(&mut self, index: usize) -> Box<Leaf<K, V>> {
        debug_assert!(self.has_leaves());
        unsafe { self.children.remove(index).unwrap_leaf() }
    }

    pub(crate) fn remove_last_branch(&mut self) -> Box<Branch<K, V>> {
        debug_assert!(self.has_branches());
        unsafe { self.children.pop_back().unwrap_branch() }
    }

    // fn remove_last_leaf(&mut self) -> Box<Leaf<K, V>> {
    //     debug_assert!(self.has_leaves());
    //     unsafe { self.children.pop_back().unwrap_leaf() }
    // }

    fn split(mut self: Box<Self>) -> (Box<Branch<K, V>>, Box<Branch<K, V>>) {
        let half = self.keys.len() / 2;
        let left = Box::new(Branch {
            height: self.height,
            keys: Chunk::from_front(&mut self.keys, half),
            children: Chunk::from_front(&mut self.children, half),
        });
        (left, self)
    }
}

impl<K, V> Branch<K, V>
where
    K: Ord + Clone,
{
    pub(crate) fn unit(height: usize, leaf: Box<Leaf<K, V>>) -> Self {
        Branch {
            height,
            keys: Chunk::unit(leaf.highest().clone()),
            children: Chunk::unit(leaf.into()),
        }
    }

    // For benchmarking: lookup with a linear search instead of binary.
    pub(crate) fn get_linear(&self, key: &K) -> Option<&V> {
        let mut ptr = self;
        loop {
            if let Some(index) = find_key_linear(&ptr.keys, key) {
                if ptr.height > 1 {
                    ptr = ptr.get_branch(index);
                } else {
                    return ptr.get_leaf(index).get_linear(key);
                }
            } else {
                return None;
            }
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        let mut ptr = self;
        loop {
            if let Some(index) = find_key(&ptr.keys, key) {
                if ptr.height > 1 {
                    ptr = ptr.get_branch(index);
                } else {
                    return ptr.get_leaf(index).get(key);
                }
            } else {
                return None;
            }
        }
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.leaf_for_mut(key, None)
            .and_then(|leaf| leaf.get_mut(key))
    }

    /// Find the closest leaf for key `K`.
    ///
    /// The leaf found is the closest one to the key, such that the lowest key in the leaf is guaranteed
    /// to be less than or equal to `K`, or to be the lowest key in the set, and the highest key is guaranteed
    /// to be greater than or equal to `K`. It is not guaranteed to contain `K`, but it's guaranteed to
    /// be where `K` would be if it's in the set.
    ///
    /// It may return `None`, which means the highest key in the set is lower than `K`, or the set is empty.
    ///
    /// If you provide the `path` argument, the branches walked to get to the leaf will be pushed to it in order,
    /// starting with the root.
    #[inline]
    pub(crate) fn leaf_for<'a>(
        &'a self,
        key: &K,
        mut path: Option<&mut Path<'a, K, V>>,
    ) -> Option<&'a Leaf<K, V>> {
        let mut branch = self;

        loop {
            if branch.is_empty() {
                return None;
            }
            if let Some(index) = find_key(&branch.keys, key) {
                if let Some(ref mut path) = path {
                    path.push_back((&*branch, index as isize));
                }
                if branch.height > 1 {
                    branch = branch.get_branch(index);
                } else {
                    return Some(branch.get_leaf(index));
                }
            } else {
                return None;
            }
        }
    }

    /// Find the closest leaf for key `K` as a mutable reference.
    ///
    /// See `leaf_for` for details.
    #[inline]
    pub(crate) fn leaf_for_mut<'a>(
        &'a mut self,
        key: &K,
        mut path: Option<&mut PathMut<K, V>>,
    ) -> Option<&'a mut Leaf<K, V>> {
        let mut branch = self;
        loop {
            if branch.is_empty() {
                return None;
            }
            if let Some(index) = find_key(&branch.keys, key) {
                if let Some(ref mut path) = path {
                    path.push_back((&mut *branch, index));
                }
                if branch.height > 1 {
                    branch = branch.get_branch_mut(index);
                } else {
                    return Some(branch.get_leaf_mut(index));
                }
            } else {
                return None;
            }
        }
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> InsertResult<K, V> {
        // TODO: this algorithm could benefit from the addition of neighbour
        // checking to reduce splitting.
        if let Some(index) = find_key(&self.keys, &key) {
            let (key, value) = {
                let result = if self.has_branches() {
                    self.get_branch_mut(index).insert(key, value)
                } else {
                    self.get_leaf_mut(index).insert(key, value)
                };
                match result {
                    InsertResult::Full(key, value) => (key, value),
                    result => return result,
                }
            };
            // Fall through from match = leaf is full and needs to be split.
            if self.is_full() {
                InsertResult::Full(key, value)
            } else if self.has_branches() {
                let (left, right) = self.remove_branch(index).split();
                self.keys.insert(index, left.highest().clone());
                self.children
                    .insert_from(index, vec![left.into(), right.into()]);
                self.insert(key, value)
            } else {
                let (left, right) = self.remove_leaf(index).split();
                self.keys.insert(index, left.highest().clone());
                self.children
                    .insert_from(index, vec![left.into(), right.into()]);
                self.insert(key, value)
            }
        } else {
            let end_index = self.keys.len() - 1;
            let (key, value) = {
                if self.has_branches() {
                    self.keys[end_index] = key.clone();
                    match self.get_branch_mut(end_index).insert(key, value) {
                        InsertResult::Full(key, value) => (key, value),
                        result => return result,
                    }
                } else {
                    let leaf = self.get_leaf_mut(end_index);
                    if !leaf.is_full() {
                        leaf.keys.push_back(key.clone());
                        leaf.values.push_back(value);
                        self.keys[end_index] = key;
                        return InsertResult::Added;
                    }
                    (key, value)
                }
            };
            if self.is_full() {
                InsertResult::Full(key, value)
            } else if self.has_branches() {
                let (left, right) = self.remove_last_branch().split();
                self.keys
                    .insert(self.keys.len() - 1, left.highest().clone());
                self.children.push_back(left.into());
                self.children.push_back(right.into());
                self.insert(key, value)
            } else {
                let leaf = Box::new(Leaf {
                    keys: Chunk::unit(key.clone()),
                    values: Chunk::unit(value),
                });
                self.keys.push_back(key);
                self.children.push_back(leaf.into());
                InsertResult::Added
            }
        }
    }

    pub(crate) fn remove(&mut self, key: &K) -> RemoveResult<K, V> {
        // BIG TODO:
        // This implementation doesn't deal with underfull nodes, on the theory that the tree
        // can be sufficiently balanced through insertion. This theory may not hold, and we
        // may need to either balance it on every deletion, or arrange to have the tree
        // periodically rebalanced through some other mechanism. It might be useful if so
        // for this method to record somewhere which nodes have become underfull, in order to
        // avoid having to rebalance the full tree.
        if let Some(index) = find_key(&self.keys, &key) {
            let result = if self.has_branches() {
                self.get_branch_mut(index).remove(key)
            } else {
                self.get_leaf_mut(index).remove(key)
            };
            match result {
                RemoveResult::DeletedAndEmpty(key, value) => {
                    self.keys.remove(index);
                    if self.has_branches() {
                        self.remove_branch(index);
                    } else {
                        self.remove_leaf(index);
                    }
                    if self.is_empty() {
                        RemoveResult::DeletedAndEmpty(key, value)
                    } else {
                        RemoveResult::Deleted(key, value)
                    }
                }
                result => result,
            }
        } else {
            RemoveResult::NotHere
        }
    }
}

impl<K, V> Branch<K, V>
where
    K: Debug,
    V: Debug,
{
    fn tree_fmt(&self, f: &mut Formatter<'_>, level: usize) -> Result<(), Error> {
        let mut indent = String::new();
        for _ in 0..level {
            indent += "    ";
        }
        writeln!(f, "{}Branch(height = {})", indent, self.height)?;
        for (index, key) in self.keys.iter().enumerate() {
            if self.has_branches() {
                writeln!(f, "{}  [{:?}]:", indent, key)?;
                self.get_branch(index).tree_fmt(f, level + 1)?;
            } else {
                writeln!(f, "{}  [{:?}]: {:?}", indent, key, self.get_leaf(index))?;
            }
        }
        Ok(())
    }
}

impl<K, V> Debug for Branch<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        self.tree_fmt(f, 0)
    }
}

// Never leak this monster to the rest of the crate.
mod node {
    use crate::{branch::Branch, leaf::Leaf};
    use std::{marker::PhantomData, ptr::NonNull};

    pub(crate) struct Node<K, V> {
        types: PhantomData<(K, V)>,
        node: NonNull<()>,
    }

    impl<K, V> Drop for Node<K, V> {
        fn drop(&mut self) {
            // Nodes should never be dropped directly.
            // Branch has to make sure they're dropped correctly,
            // because only Branch knows whether they contain Leaves or Branches.
            unreachable!(
                "PalmTree: tried to drop a Node pointer directly, this should never happen"
            )
        }
    }

    impl<K, V> From<Box<Leaf<K, V>>> for Node<K, V> {
        fn from(node: Box<Leaf<K, V>>) -> Self {
            Self {
                types: PhantomData,
                // TODO this is better expressed with Box::into_raw_non_null, when that stabilises,
                // no need for an unsafe block here when it does.
                node: unsafe { NonNull::new_unchecked(Box::into_raw(node).cast()) },
            }
        }
    }

    impl<K, V> From<Box<Branch<K, V>>> for Node<K, V> {
        fn from(node: Box<Branch<K, V>>) -> Self {
            Self {
                types: PhantomData,
                node: unsafe { NonNull::new_unchecked(Box::into_raw(node).cast()) },
            }
        }
    }

    impl<K, V> Node<K, V> {
        pub(crate) unsafe fn unwrap_branch(self) -> Box<Branch<K, V>> {
            let out = Box::from_raw(self.node.as_ptr().cast());
            std::mem::forget(self);
            out
        }

        pub(crate) unsafe fn unwrap_leaf(self) -> Box<Leaf<K, V>> {
            let out = Box::from_raw(self.node.as_ptr().cast());
            std::mem::forget(self);
            out
        }

        pub(crate) unsafe fn as_branch(&self) -> &Branch<K, V> {
            let ptr: *const Branch<K, V> = self.node.cast().as_ptr();
            ptr.as_ref().unwrap()
        }

        pub(crate) unsafe fn as_leaf(&self) -> &Leaf<K, V> {
            let ptr: *const Leaf<K, V> = self.node.cast().as_ptr();
            ptr.as_ref().unwrap()
        }

        pub(crate) unsafe fn as_branch_mut(&mut self) -> &mut Branch<K, V> {
            let ptr: *mut Branch<K, V> = self.node.cast().as_ptr();
            ptr.as_mut().unwrap()
        }

        pub(crate) unsafe fn as_leaf_mut(&mut self) -> &mut Leaf<K, V> {
            let ptr: *mut Leaf<K, V> = self.node.cast().as_ptr();
            ptr.as_mut().unwrap()
        }
    }
}
