use crate::{branch::Branch, leaf::Leaf};
use sized_chunks::types::ChunkLength;
use std::{
    fmt::{Debug, Error, Formatter},
    marker::PhantomData,
    ptr::NonNull,
};
use typenum::{IsGreater, U3};

pub struct Node<K, V, B, L> {
    types: PhantomData<(K, V, B, L)>,
    node: NonNull<()>,
}

impl<K, V, B, L> Drop for Node<K, V, B, L> {
    fn drop(&mut self) {
        // Nodes should never be dropped directly.
        // Branch has to make sure they're dropped correctly,
        // because only Branch knows whether they contain Leaves or Branches.
        unreachable!("PalmTree: tried to drop a Node pointer directly, this should never happen")
    }
}

impl<K, V, B, L> From<Box<Leaf<K, V, L>>> for Node<K, V, B, L>
where
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn from(node: Box<Leaf<K, V, L>>) -> Self {
        let ptr: NonNull<Leaf<K, V, L>> = Box::leak(node).into();
        Self {
            types: PhantomData,
            node: ptr.cast(),
        }
    }
}

impl<K, V, B, L> From<Box<Branch<K, V, B, L>>> for Node<K, V, B, L>
where
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn from(node: Box<Branch<K, V, B, L>>) -> Self {
        let ptr: NonNull<Branch<K, V, B, L>> = Box::leak(node).into();
        Self {
            types: PhantomData,
            node: ptr.cast(),
        }
    }
}

impl<K, V, B, L> Node<K, V, B, L> {
    pub(crate) unsafe fn unwrap_branch(self) -> Box<Branch<K, V, B, L>>
    where
        B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
        L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
    {
        let out = Box::from_raw(self.node.as_ptr().cast());
        std::mem::forget(self);
        out
    }

    pub(crate) unsafe fn unwrap_leaf(self) -> Box<Leaf<K, V, L>>
    where
        B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
        L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
    {
        let out = Box::from_raw(self.node.as_ptr().cast());
        std::mem::forget(self);
        out
    }

    pub(crate) unsafe fn as_branch(&self) -> &Branch<K, V, B, L>
    where
        B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
        L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
    {
        let ptr: *const Branch<K, V, B, L> = self.node.cast().as_ptr();
        ptr.as_ref().unwrap()
    }

    pub(crate) unsafe fn as_leaf(&self) -> &Leaf<K, V, L>
    where
        B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
        L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
    {
        let ptr: *const Leaf<K, V, L> = self.node.cast().as_ptr();
        ptr.as_ref().unwrap()
    }

    pub(crate) unsafe fn as_branch_mut(&mut self) -> &mut Branch<K, V, B, L>
    where
        B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
        L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
    {
        let ptr: *mut Branch<K, V, B, L> = self.node.cast().as_ptr();
        ptr.as_mut().unwrap()
    }

    pub(crate) unsafe fn as_leaf_mut(&mut self) -> &mut Leaf<K, V, L>
    where
        B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
        L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
    {
        let ptr: *mut Leaf<K, V, L> = self.node.cast().as_ptr();
        ptr.as_mut().unwrap()
    }
}

impl<K, V, B, L> Debug for Node<K, V, B, L>
where
    B: ChunkLength<K> + ChunkLength<Node<K, V, B, L>> + IsGreater<U3>,
    L: ChunkLength<K> + ChunkLength<V> + IsGreater<U3>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "Node[...]")
    }
}
