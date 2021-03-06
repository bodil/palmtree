use crate::{
    array::Array,
    config::TreeConfig,
    leaf::Leaf,
    pointer::Pointer,
    search::{find_key, find_key_linear},
    InsertResult,
};
use node::Node;
use std::fmt::{Debug, Error, Formatter};
use typenum::Unsigned;

// Never leak this monster to the rest of the crate.
pub(crate) mod node;

/// A branch node holds mappings of high keys to child nodes.
pub(crate) struct Branch<K, V, C>
where
    C: TreeConfig<K, V>,
{
    has_branches: bool,
    length: usize,
    keys: Array<K, C::BranchSize>,
    children: Array<Node<K, V, C>, C::BranchSize>,
}

impl<K, V, C> Drop for Branch<K, V, C>
where
    C: TreeConfig<K, V>,
{
    fn drop(&mut self) {
        unsafe {
            self.keys.drop(self.length);
            while self.length > 0 {
                // The `Node` type can't drop itself because it doesn't know
                // whether it's a Branch or a Leaf, so we *must* drop every `Node`
                // from the `Branch` it's stored in.
                let node = self.children.pop(self.length);
                self.length -= 1;
                if self.has_branches() {
                    node.unwrap_branch();
                } else {
                    node.unwrap_leaf();
                }
            }
        }
    }
}

impl<K, V, C> Clone for Branch<K, V, C>
where
    K: Clone,
    V: Clone,
    C: TreeConfig<K, V>,
{
    fn clone(&self) -> Self {
        let children = unsafe {
            if self.has_branches() {
                self.children.clone_with(self.length, |node| {
                    Pointer::new(node.as_branch().clone()).into()
                })
            } else {
                self.children.clone_with(self.length, |node| {
                    Pointer::new(node.as_leaf().clone()).into()
                })
            }
        };
        Self {
            has_branches: self.has_branches,
            length: self.length,
            keys: unsafe { self.keys.clone(self.length) },
            children,
        }
    }
}

impl<K, V, C> Branch<K, V, C>
where
    C: TreeConfig<K, V>,
{
    #[inline(always)]
    pub(crate) fn new(has_branches: bool) -> Self {
        Branch {
            has_branches,
            length: 0,
            keys: Array::new(),
            children: Array::new(),
        }
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.length
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub(crate) fn is_full(&self) -> bool {
        self.len() == C::BranchSize::USIZE
    }

    #[inline(always)]
    pub(crate) fn highest(&self) -> &K {
        &self.keys()[self.len() - 1]
    }

    #[inline(always)]
    pub(crate) fn has_leaves(&self) -> bool {
        !self.has_branches()
    }

    #[inline(always)]
    pub(crate) fn has_branches(&self) -> bool {
        self.has_branches
    }

    #[inline(always)]
    pub(crate) fn keys(&self) -> &[K] {
        unsafe { self.keys.deref(self.length) }
    }

    #[inline(always)]
    pub(crate) fn keys_mut(&mut self) -> &mut [K] {
        unsafe { self.keys.deref_mut(self.length) }
    }

    #[inline(always)]
    fn children(&self) -> &[Node<K, V, C>] {
        unsafe { self.children.deref(self.length) }
    }

    #[inline(always)]
    fn children_mut(&mut self) -> &mut [Node<K, V, C>] {
        unsafe { self.children.deref_mut(self.length) }
    }

    #[inline(always)]
    pub(crate) fn get_branch(&self, index: usize) -> &Self {
        debug_assert!(self.has_branches());
        unsafe { self.children()[index].as_branch() }
    }

    #[inline(always)]
    pub(crate) unsafe fn get_branch_unchecked(&self, index: usize) -> &Self {
        debug_assert!(self.has_branches());
        debug_assert!(self.len() > index);
        self.children().get_unchecked(index).as_branch()
    }

    #[inline(always)]
    pub(crate) fn get_leaf(&self, index: usize) -> &Leaf<K, V, C> {
        debug_assert!(self.has_leaves());
        unsafe { self.children()[index].as_leaf() }
    }

    #[inline(always)]
    pub(crate) unsafe fn get_leaf_unchecked(&self, index: usize) -> &Leaf<K, V, C> {
        debug_assert!(self.has_leaves());
        debug_assert!(self.len() > index);
        self.children().get_unchecked(index).as_leaf()
    }

    #[inline(always)]
    pub(crate) fn get_branch_mut(&mut self, index: usize) -> &mut Self
    where
        K: Clone,
        V: Clone,
    {
        debug_assert!(self.has_branches());
        unsafe { self.children_mut()[index].as_branch_mut() }
    }

    #[inline(always)]
    pub(crate) fn get_leaf_mut(&mut self, index: usize) -> &mut Leaf<K, V, C>
    where
        K: Clone,
        V: Clone,
    {
        debug_assert!(self.has_leaves());
        unsafe { self.children_mut()[index].as_leaf_mut() }
    }

    #[inline(always)]
    pub(crate) fn push_branch(&mut self, key: K, branch: Pointer<Self, C::PointerKind>) {
        debug_assert!(self.has_branches());
        debug_assert!(!self.is_full());
        unsafe {
            self.keys.push(self.length, key);
            self.children.push(self.length, branch.into());
        }
        self.length += 1;
    }

    #[inline(always)]
    pub(crate) fn push_leaf(&mut self, key: K, leaf: Pointer<Leaf<K, V, C>, C::PointerKind>) {
        debug_assert!(self.has_leaves());
        debug_assert!(!self.is_full());
        unsafe {
            self.keys.push(self.length, key);
            self.children.push(self.length, leaf.into());
        }
        self.length += 1;
    }

    #[inline(always)]
    pub(crate) fn remove_branch(&mut self, index: usize) -> (K, Pointer<Self, C::PointerKind>) {
        debug_assert!(self.has_branches());
        debug_assert!(index < self.length);
        let result = unsafe {
            (
                self.keys.remove(self.length, index),
                self.children.remove(self.length, index).unwrap_branch(),
            )
        };
        self.length -= 1;
        result
    }

    #[inline(always)]
    pub(crate) fn remove_leaf(
        &mut self,
        index: usize,
    ) -> (K, Pointer<Leaf<K, V, C>, C::PointerKind>) {
        debug_assert!(self.has_leaves());
        debug_assert!(index < self.length);
        let result = unsafe {
            (
                self.keys.remove(self.length, index),
                self.children.remove(self.length, index).unwrap_leaf(),
            )
        };
        self.length -= 1;
        result
    }

    #[inline(always)]
    pub(crate) fn remove_last_branch(&mut self) -> (K, Pointer<Self, C::PointerKind>) {
        debug_assert!(self.has_branches());
        debug_assert!(!self.is_empty());
        let result = unsafe {
            (
                self.keys.pop(self.length),
                self.children.pop(self.length).unwrap_branch(),
            )
        };
        self.length -= 1;
        result
    }

    #[inline(always)]
    pub(crate) fn push_branch_pair(
        &mut self,
        left_key: K,
        left: Pointer<Self, C::PointerKind>,
        right_key: K,
        right: Pointer<Self, C::PointerKind>,
    ) {
        debug_assert!(self.has_branches());
        debug_assert!(self.len() + 2 <= C::BranchSize::USIZE);
        unsafe {
            self.keys
                .insert_pair(self.length, self.length, left_key, right_key);
            self.children
                .insert_pair(self.length, self.length, left.into(), right.into());
        }
        self.length += 2;
    }

    #[inline(always)]
    pub(crate) fn insert_branch_pair(
        &mut self,
        index: usize,
        left_key: K,
        left: Pointer<Self, C::PointerKind>,
        right_key: K,
        right: Pointer<Self, C::PointerKind>,
    ) {
        debug_assert!(self.has_branches());
        debug_assert!(self.len() + 2 <= C::BranchSize::USIZE);
        unsafe {
            self.keys
                .insert_pair(self.length, index, left_key, right_key);
            self.children
                .insert_pair(self.length, index, left.into(), right.into());
        }
        self.length += 2;
    }

    #[inline(always)]
    pub(crate) fn insert_leaf_pair(
        &mut self,
        index: usize,
        left_key: K,
        left: Pointer<Leaf<K, V, C>, C::PointerKind>,
        right_key: K,
        right: Pointer<Leaf<K, V, C>, C::PointerKind>,
    ) {
        debug_assert!(self.has_leaves());
        debug_assert!(self.len() + 2 <= C::BranchSize::USIZE);
        unsafe {
            self.keys
                .insert_pair(self.length, index, left_key, right_key);
            self.children
                .insert_pair(self.length, index, left.into(), right.into());
        }
        self.length += 2;
    }

    pub(crate) fn split(
        mut this: Pointer<Self, C::PointerKind>,
    ) -> (Pointer<Self, C::PointerKind>, Pointer<Self, C::PointerKind>)
    where
        K: Clone,
        V: Clone,
    {
        let right = {
            let this = Pointer::make_mut(&mut this);
            let half = this.len() / 2;
            let right = Pointer::new(Branch {
                has_branches: this.has_branches,
                length: half,
                keys: unsafe { Array::steal_from(&mut this.keys, this.length, half) },
                children: unsafe { Array::steal_from(&mut this.children, this.length, half) },
            });
            this.length -= half;
            right
        };
        (this, right)
    }
}

impl<K, V, C> Branch<K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    pub(crate) fn unit(leaf: Pointer<Leaf<K, V, C>, C::PointerKind>) -> Self {
        Branch {
            has_branches: false,
            length: 1,
            keys: unsafe { Array::unit(leaf.highest().clone()) },
            children: unsafe { Array::unit(leaf.into()) },
        }
    }

    // For benchmarking: lookup with a linear search instead of binary.
    pub(crate) fn get_linear(&self, key: &K) -> Option<&V> {
        let mut branch = self;
        loop {
            if let Some(index) = find_key_linear(branch.keys(), key) {
                if branch.has_branches() {
                    branch = branch.get_branch(index);
                } else {
                    return branch.get_leaf(index).get_linear(key);
                }
            } else {
                return None;
            }
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        let mut branch = self;
        loop {
            if let Some(index) = find_key(branch.keys(), key) {
                if branch.has_branches() {
                    branch = branch.get_branch(index);
                } else {
                    return branch.get_leaf(index).get(key);
                }
            } else {
                return None;
            }
        }
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V>
    where
        V: Clone,
    {
        let mut branch = self;
        loop {
            if branch.is_empty() {
                return None;
            }
            if let Some(index) = find_key(branch.keys(), key) {
                if branch.has_branches() {
                    branch = branch.get_branch_mut(index);
                } else {
                    return branch.get_leaf_mut(index).get_mut(key);
                }
            } else {
                return None;
            }
        }
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> InsertResult<K, V>
    where
        V: Clone,
    {
        // TODO: this algorithm could benefit from the addition of neighbour
        // checking to reduce splitting.
        if let Some(index) = find_key(self.keys(), &key) {
            // We have found a key match, attempt to insert into the matching child.
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
            // Fall through from match = child is full and needs to be split.
            if self.is_full() {
                // Current branch is full, needs to split further up.
                InsertResult::Full(key, value)
            } else if self.has_branches() {
                // Split the child branch and retry insertion from here.
                // FIXME should determine which of the split branches to insert into instead of rechecking from the parent branch.
                // Same for leaf splitting below, and splitting in >max case further below.
                let (removed_key, removed_branch) = self.remove_branch(index);
                let (left, right) = Self::split(removed_branch);
                self.insert_branch_pair(index, left.highest().clone(), left, removed_key, right);
                self.insert(key, value)
            } else {
                let (removed_key, removed_leaf) = self.remove_leaf(index);
                let (left, right) = Leaf::split(removed_leaf);
                self.insert_leaf_pair(index, left.highest().clone(), left, removed_key, right);
                self.insert(key, value)
            }
        } else {
            // No key match, which means the key is higher than the current max, so we insert along the right edge.
            let end_index = self.len() - 1;
            let (key, value) = {
                if self.has_branches() {
                    self.keys_mut()[end_index] = key.clone();
                    match self.get_branch_mut(end_index).insert(key, value) {
                        InsertResult::Full(key, value) => (key, value),
                        result => return result,
                    }
                } else {
                    let leaf = self.get_leaf_mut(end_index);
                    if !leaf.is_full() {
                        unsafe { leaf.push_unchecked(key.clone(), value) };
                        self.keys_mut()[end_index] = key;
                        return InsertResult::Added;
                    }
                    (key, value)
                }
            };
            if self.is_full() {
                InsertResult::Full(key, value)
            } else if self.has_branches() {
                let (removed_key, removed_branch) = self.remove_last_branch();
                let (left, right) = Self::split(removed_branch);
                self.push_branch_pair(left.highest().clone(), left, removed_key, right);
                self.insert(key, value)
            } else {
                let leaf = Pointer::new(Leaf::unit(key.clone(), value));
                self.push_leaf(key, leaf);
                InsertResult::Added
            }
        }
    }
}

impl<K, V, C> Branch<K, V, C>
where
    K: Clone + Debug,
    V: Clone + Debug,
    C: TreeConfig<K, V>,
{
    fn tree_fmt(&self, f: &mut Formatter<'_>, level: usize) -> Result<(), Error> {
        let mut indent = String::new();
        for _ in 0..level {
            indent += "    ";
        }
        writeln!(
            f,
            "{}Branch(has_branches = {})",
            indent,
            self.has_branches()
        )?;
        for (index, key) in self.keys().iter().enumerate() {
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

impl<K, V, C> Debug for Branch<K, V, C>
where
    K: Clone + Debug,
    V: Clone + Debug,
    C: TreeConfig<K, V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        self.tree_fmt(f, 0)
    }
}
