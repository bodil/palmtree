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
        unreachable!("PalmTree: tried to drop a Node pointer directly, this should never happen")
    }
}

impl<K, V> From<Box<Leaf<K, V>>> for Node<K, V> {
    fn from(node: Box<Leaf<K, V>>) -> Self {
        let ptr: NonNull<Leaf<K, V>> = Box::leak(node).into();
        Self {
            types: PhantomData,
            node: ptr.cast(),
        }
    }
}

impl<K, V> From<Box<Branch<K, V>>> for Node<K, V> {
    fn from(node: Box<Branch<K, V>>) -> Self {
        let ptr: NonNull<Branch<K, V>> = Box::leak(node).into();
        Self {
            types: PhantomData,
            node: ptr.cast(),
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
