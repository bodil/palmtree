use crate::{
    leaf::Leaf,
    search::{find_key, find_key_linear},
    types::{MaxHeight, NodeSize},
};
use node::Node;
use sized_chunks::Chunk;
use std::fmt::{Debug, Error, Formatter};
use typenum::{Unsigned, U2};

// Never leak this monster to the rest of the crate.
mod node;

/// A branch node holds mappings of high keys to child nodes.
pub(crate) struct Branch<K, V> {
    height: usize,
    pub(crate) keys: Chunk<K, NodeSize>,
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

impl<K, V> Clone for Branch<K, V>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        let mut children = Chunk::new();
        if self.has_branches() {
            for node in &self.children {
                let branch = Box::new(unsafe { node.as_branch() }.clone());
                children.push_back(branch.into());
            }
        } else {
            for node in &self.children {
                let leaf = Box::new(unsafe { node.as_leaf() }.clone());
                children.push_back(leaf.into());
            }
        }
        Self {
            height: self.height,
            keys: self.keys.clone(),
            children,
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

    pub(crate) fn keys(&self) -> &[K] {
        &self.keys
    }

    pub(crate) fn get_branch(&self, index: usize) -> &Branch<K, V> {
        debug_assert!(self.has_branches());
        unsafe { self.children[index].as_branch() }
    }

    pub(crate) fn get_leaf(&self, index: usize) -> &Leaf<K, V> {
        debug_assert!(self.has_leaves());
        unsafe { self.children[index].as_leaf() }
    }

    pub(crate) fn get_branch_mut(&mut self, index: usize) -> &mut Branch<K, V> {
        debug_assert!(self.has_branches());
        unsafe { self.children[index].as_branch_mut() }
    }

    pub(crate) fn get_leaf_mut(&mut self, index: usize) -> &mut Leaf<K, V> {
        debug_assert!(self.has_leaves());
        unsafe { self.children[index].as_leaf_mut() }
    }

    pub(crate) fn push_key(&mut self, key: K) {
        self.keys.push_back(key)
    }

    pub(crate) fn insert_key(&mut self, index: usize, key: K) {
        self.keys.insert(index, key)
    }

    pub(crate) fn push_branch(&mut self, branch: Box<Branch<K, V>>) {
        debug_assert!(self.has_branches());
        self.children.push_back(branch.into())
    }

    pub(crate) fn push_leaf(&mut self, leaf: Box<Leaf<K, V>>) {
        debug_assert!(self.has_leaves());
        self.children.push_back(leaf.into())
    }

    pub(crate) fn remove_key(&mut self, index: usize) -> K {
        self.keys.remove(index)
    }

    pub(crate) fn remove_branch(&mut self, index: usize) -> Box<Branch<K, V>> {
        debug_assert!(self.has_branches());
        unsafe { self.children.remove(index).unwrap_branch() }
    }

    pub(crate) fn remove_leaf(&mut self, index: usize) -> Box<Leaf<K, V>> {
        debug_assert!(self.has_leaves());
        unsafe { self.children.remove(index).unwrap_leaf() }
    }

    pub(crate) fn remove_last_branch(&mut self) -> Box<Branch<K, V>> {
        debug_assert!(self.has_branches());
        unsafe { self.children.pop_back().unwrap_branch() }
    }

    pub(crate) fn insert_branch_pair(
        &mut self,
        index: usize,
        left: Box<Branch<K, V>>,
        right: Box<Branch<K, V>>,
    ) {
        debug_assert!(self.has_branches());
        self.children
            .insert_from(index, Chunk::<_, U2>::pair(left.into(), right.into()));
    }

    pub(crate) fn insert_leaf_pair(
        &mut self,
        index: usize,
        left: Box<Leaf<K, V>>,
        right: Box<Leaf<K, V>>,
    ) {
        debug_assert!(self.has_leaves());
        self.children
            .insert_from(index, Chunk::<_, U2>::pair(left.into(), right.into()));
    }

    pub(crate) fn split(mut self: Box<Self>) -> (Box<Branch<K, V>>, Box<Branch<K, V>>) {
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
    pub(crate) fn unit(leaf: Box<Leaf<K, V>>) -> Self {
        Branch {
            height: 1,
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
        let mut branch = self;
        loop {
            if branch.is_empty() {
                return None;
            }
            if let Some(index) = find_key(&branch.keys, key) {
                if branch.height > 1 {
                    branch = branch.get_branch_mut(index);
                } else {
                    return branch.get_leaf_mut(index).get_mut(key);
                }
            } else {
                return None;
            }
        }
    }

    // fn remove_result(&mut self, index: usize, result: RemoveResult<K, V>) -> RemoveResult<K, V> {
    //     match result {
    //         RemoveResult::DeletedAndEmpty(key, value) => {
    //             self.keys.remove(index);
    //             if self.has_branches() {
    //                 self.remove_branch(index);
    //             } else {
    //                 self.remove_leaf(index);
    //             }
    //             if self.is_empty() {
    //                 RemoveResult::DeletedAndEmpty(key, value)
    //             } else {
    //                 RemoveResult::Deleted(key, value)
    //             }
    //         }
    //         result => result,
    //     }
    // }

    // pub(crate) fn remove(&mut self, key: &K) -> RemoveResult<K, V> {
    //     // BIG TODO:
    //     // This implementation doesn't deal with underfull nodes, on the theory that the tree
    //     // can be sufficiently balanced through insertion. This theory may not hold, and we
    //     // may need to either balance it on every deletion, or arrange to have the tree
    //     // periodically rebalanced through some other mechanism. It might be useful if so
    //     // for this method to record somewhere which nodes have become underfull, in order to
    //     // avoid having to rebalance the full tree.
    //     if let Some(index) = find_key(&self.keys, &key) {
    //         let result = if self.has_branches() {
    //             self.get_branch_mut(index).remove(key)
    //         } else {
    //             self.get_leaf_mut(index).remove(key)
    //         };
    //         self.remove_result(index, result)
    //     } else {
    //         RemoveResult::NotHere
    //     }
    // }
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
