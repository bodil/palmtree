use crate::{
    leaf::Leaf,
    search::{find_key, find_key_linear},
    types::{InsertResult, NodeSize, RemoveResult},
};
use sized_chunks::Chunk;
use std::fmt::{Debug, Error, Formatter};

/// A branch node holds mappings of high keys to child nodes.
pub(crate) struct Branch<K, V> {
    pub(crate) height: usize,
    pub(crate) keys: Chunk<K, NodeSize>,
    pub(crate) children: Chunk<Node<K, V>, NodeSize>,
}

pub(crate) enum Node<K, V> {
    Branch(Box<Branch<K, V>>),
    Leaf(Box<Leaf<K, V>>),
}

impl<K, V> From<Leaf<K, V>> for Node<K, V> {
    fn from(node: Leaf<K, V>) -> Self {
        Box::new(node).into()
    }
}

impl<K, V> From<Box<Leaf<K, V>>> for Node<K, V> {
    fn from(node: Box<Leaf<K, V>>) -> Self {
        Self::Leaf(node)
    }
}

impl<K, V> From<Branch<K, V>> for Node<K, V> {
    fn from(node: Branch<K, V>) -> Self {
        Box::new(node).into()
    }
}

impl<K, V> From<Box<Branch<K, V>>> for Node<K, V> {
    fn from(node: Box<Branch<K, V>>) -> Self {
        Self::Branch(node)
    }
}

impl<K, V> Node<K, V> {
    fn unwrap_branch(self) -> Box<Branch<K, V>> {
        match self {
            Node::Branch(branch) => branch,
            _ => panic!("unwrap_branch on not-branch"),
        }
    }

    fn unwrap_leaf(self) -> Box<Leaf<K, V>> {
        match self {
            Node::Leaf(leaf) => leaf,
            _ => panic!("unwrap_leaf on not-leaf"),
        }
    }
}

impl<K, V> Branch<K, V> {
    pub(crate) fn new(height: usize) -> Self {
        Branch {
            height,
            keys: Chunk::new(),
            children: Chunk::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.keys.is_full()
    }

    pub(crate) fn highest(&self) -> &K {
        self.keys.last().unwrap()
    }

    fn split(mut self) -> (Branch<K, V>, Branch<K, V>) {
        let half = self.keys.len() / 2;
        let left = Branch {
            height: self.height,
            keys: Chunk::from_front(&mut self.keys, half),
            children: Chunk::from_front(&mut self.children, half),
        };
        (left, self)
    }

    pub(crate) fn start_path(&self) -> (Vec<(&Branch<K, V>, isize)>, Option<&Leaf<K, V>>, usize) {
        let mut branch = self;
        let mut path = Vec::new();
        loop {
            path.push((branch, 0));
            match branch.children[0] {
                Node::Branch(ref child) => {
                    branch = child;
                }
                Node::Leaf(ref leaf) => {
                    return (path, Some(leaf), 0);
                }
            }
        }
    }

    pub(crate) fn end_path(&self) -> (Vec<(&Branch<K, V>, isize)>, Option<&Leaf<K, V>>, usize) {
        let mut branch = self;
        let mut path = Vec::new();
        loop {
            let index = branch.keys.len() - 1;
            path.push((branch, index as isize));
            match branch.children[index] {
                Node::Branch(ref child) => {
                    branch = child;
                }
                Node::Leaf(ref leaf) => {
                    return (path, Some(leaf), leaf.keys.len() - 1);
                }
            }
        }
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
                match &ptr.children[index] {
                    Node::Leaf(leaf) => return leaf.get_linear(key),
                    Node::Branch(child) => ptr = child,
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
                match &ptr.children[index] {
                    Node::Leaf(leaf) => return leaf.get(key),
                    Node::Branch(child) => ptr = child,
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
        mut path: Option<&mut Vec<(&'a Branch<K, V>, isize)>>,
    ) -> Option<&'a Leaf<K, V>> {
        let mut ptr = self;
        loop {
            if let Some(index) = find_key(&ptr.keys, key) {
                if let Some(ref mut path) = path {
                    path.push((&*ptr, index as isize));
                }
                match &ptr.children[index] {
                    Node::Leaf(leaf) => return Some(leaf),
                    Node::Branch(child) => {
                        ptr = child;
                    }
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
        mut path: Option<&mut Vec<(*mut Branch<K, V>, usize)>>,
    ) -> Option<&'a mut Leaf<K, V>> {
        let mut ptr = self;
        loop {
            if let Some(index) = find_key(&ptr.keys, key) {
                if let Some(ref mut path) = path {
                    path.push((&mut *ptr, index));
                }
                match &mut ptr.children[index] {
                    Node::Leaf(leaf) => return Some(leaf),
                    Node::Branch(child) => ptr = child,
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
            let (split_child, key, value) = match &mut self.children[index] {
                Node::Branch(child) => match child.insert(key, value) {
                    InsertResult::Full(key, value) => (true, key, value),
                    result => return result,
                },
                Node::Leaf(leaf) => match leaf.insert(key, value) {
                    InsertResult::Full(key, value) => (false, key, value),
                    result => return result,
                },
            };
            // Fall through from match = leaf is full and needs to be split.
            if self.is_full() {
                InsertResult::Full(key, value)
            } else if split_child {
                let (left, right) = self.children.remove(index).unwrap_branch().split();
                self.keys.insert(index, left.highest().clone());
                self.children
                    .insert_from(index, vec![left.into(), right.into()]);
                self.insert(key, value)
            } else {
                let (left, right) = self.children.remove(index).unwrap_leaf().split();
                self.keys.insert(index, left.highest().clone());
                self.children
                    .insert_from(index, vec![left.into(), right.into()]);
                self.insert(key, value)
            }
        } else {
            let end_index = self.keys.len() - 1;
            let (split_child, key, value) = match &mut self.children[end_index] {
                Node::Branch(child) => {
                    self.keys[end_index] = key.clone();
                    match child.insert(key, value) {
                        InsertResult::Full(key, value) => (true, key, value),
                        result => return result,
                    }
                }
                Node::Leaf(leaf) => {
                    if !leaf.is_full() {
                        leaf.keys.push_back(key.clone());
                        leaf.values.push_back(value);
                        self.keys[end_index] = key;
                        return InsertResult::Added;
                    }
                    (false, key, value)
                }
            };
            if self.is_full() {
                InsertResult::Full(key, value)
            } else if split_child {
                let (left, right) = self.children.pop_back().unwrap_branch().split();
                self.keys
                    .insert(self.keys.len() - 1, left.highest().clone());
                self.children.push_back(Node::Branch(Box::new(left)));
                self.children.push_back(Node::Branch(Box::new(right)));
                self.insert(key, value)
            } else {
                let leaf = Leaf {
                    keys: Chunk::unit(key.clone()),
                    values: Chunk::unit(value),
                }
                .into();
                self.keys.push_back(key);
                self.children.push_back(leaf);
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
            let result = match &mut self.children[index] {
                Node::Leaf(ref mut leaf) => leaf.remove(key),
                Node::Branch(ref mut child) => child.remove(key),
            };
            match result {
                RemoveResult::DeletedAndEmpty(key, value) => {
                    self.keys.remove(index);
                    self.children.remove(index);
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

impl<K, V> Branch<K, V> {
    fn tree_fmt(&self, f: &mut Formatter<'_>, level: usize) -> Result<(), Error>
    where
        K: Debug,
        V: Debug,
    {
        let mut indent = String::new();
        for _ in 0..level {
            indent += "    ";
        }
        writeln!(f, "{}Branch(height = {})", indent, self.height)?;
        for (index, key) in self.keys.iter().enumerate() {
            match &self.children[index] {
                Node::Leaf(leaf) => writeln!(f, "{}  [{:?}]: {:?}", indent, key, leaf)?,
                // Node::Leaf(_leaf) => writeln!(f, "{}  [{:?}]: [...]", indent, key)?,
                Node::Branch(child) => {
                    writeln!(f, "{}  [{:?}]:", indent, key)?;
                    child.tree_fmt(f, level + 1)?;
                }
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
