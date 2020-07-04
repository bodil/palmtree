use crate::{branch::Branch, config::TreeConfig, leaf::Leaf};
use std::{
    fmt::{Debug, Error, Formatter},
    marker::PhantomData,
    ptr::NonNull,
};

pub struct Node<K, V, C>
where
    C: ?Sized,
{
    types: PhantomData<(K, V, C)>,
    node: NonNull<()>,
}

impl<K, V, C> Drop for Node<K, V, C>
where
    C: ?Sized,
{
    fn drop(&mut self) {
        // Nodes should never be dropped directly.
        // Branch has to make sure they're dropped correctly,
        // because only Branch knows whether they contain Leaves or Branches.
        unreachable!("PalmTree: tried to drop a Node pointer directly, this should never happen")
    }
}

impl<K, V, C> From<Box<Leaf<K, V, C>>> for Node<K, V, C>
where
    C: TreeConfig<K, V>,
{
    #[inline(always)]
    fn from(node: Box<Leaf<K, V, C>>) -> Self {
        let ptr: NonNull<Leaf<K, V, C>> = Box::leak(node).into();
        Self {
            types: PhantomData,
            node: ptr.cast(),
        }
    }
}

impl<K, V, C> From<Box<Branch<K, V, C>>> for Node<K, V, C>
where
    C: TreeConfig<K, V>,
{
    #[inline(always)]
    fn from(node: Box<Branch<K, V, C>>) -> Self {
        let ptr: NonNull<Branch<K, V, C>> = Box::leak(node).into();
        Self {
            types: PhantomData,
            node: ptr.cast(),
        }
    }
}

impl<K, V, C> Node<K, V, C> {
    pub(crate) unsafe fn unwrap_branch(self) -> Box<Branch<K, V, C>>
    where
        C: TreeConfig<K, V>,
    {
        let out = Box::from_raw(self.node.as_ptr().cast());
        std::mem::forget(self);
        out
    }

    pub(crate) unsafe fn unwrap_leaf(self) -> Box<Leaf<K, V, C>>
    where
        C: TreeConfig<K, V>,
    {
        let out = Box::from_raw(self.node.as_ptr().cast());
        std::mem::forget(self);
        out
    }

    #[inline(always)]
    pub(crate) unsafe fn as_branch(&self) -> &Branch<K, V, C>
    where
        C: TreeConfig<K, V>,
    {
        let ptr: *const Branch<K, V, C> = self.node.cast().as_ptr();
        &*ptr
    }

    #[inline(always)]
    pub(crate) unsafe fn as_leaf(&self) -> &Leaf<K, V, C>
    where
        C: TreeConfig<K, V>,
    {
        let ptr: *const Leaf<K, V, C> = self.node.cast().as_ptr();
        &*ptr
    }

    #[inline(always)]
    pub(crate) unsafe fn as_branch_mut(&mut self) -> &mut Branch<K, V, C>
    where
        C: TreeConfig<K, V>,
    {
        let ptr: *mut Branch<K, V, C> = self.node.cast().as_ptr();
        &mut *ptr
    }

    #[inline(always)]
    pub(crate) unsafe fn as_leaf_mut(&mut self) -> &mut Leaf<K, V, C>
    where
        C: TreeConfig<K, V>,
    {
        let ptr: *mut Leaf<K, V, C> = self.node.cast().as_ptr();
        &mut *ptr
    }
}

impl<K, V, C> Debug for Node<K, V, C>
where
    C: TreeConfig<K, V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "Node[...]")
    }
}
