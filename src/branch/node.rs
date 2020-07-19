use crate::{branch::Branch, config::TreeConfig, leaf::Leaf, pointer::Pointer};
use std::{
    fmt::{Debug, Error, Formatter},
    marker::PhantomData,
    mem::ManuallyDrop,
};

pub struct Node<K, V, C>
where
    C: ?Sized + TreeConfig<K, V>,
{
    types: PhantomData<(K, V, C)>,
    node: ManuallyDrop<Pointer<(), C::PointerKind>>,
}

impl<K, V, C> From<Pointer<Leaf<K, V, C>, C::PointerKind>> for Node<K, V, C>
where
    C: TreeConfig<K, V>,
{
    #[inline(always)]
    fn from(node: Pointer<Leaf<K, V, C>, C::PointerKind>) -> Self {
        Self {
            types: PhantomData,
            node: ManuallyDrop::new(unsafe { Pointer::cast_into(node) }),
        }
    }
}

impl<K, V, C> From<Pointer<Branch<K, V, C>, C::PointerKind>> for Node<K, V, C>
where
    C: TreeConfig<K, V>,
{
    #[inline(always)]
    fn from(node: Pointer<Branch<K, V, C>, C::PointerKind>) -> Self {
        Self {
            types: PhantomData,
            node: ManuallyDrop::new(unsafe { Pointer::cast_into(node) }),
        }
    }
}

impl<K, V, C> Node<K, V, C>
where
    C: TreeConfig<K, V>,
{
    pub(crate) unsafe fn unwrap_branch(self) -> Pointer<Branch<K, V, C>, C::PointerKind> {
        Pointer::cast_into(ManuallyDrop::into_inner(self.node))
    }

    pub(crate) unsafe fn unwrap_leaf(self) -> Pointer<Leaf<K, V, C>, C::PointerKind> {
        Pointer::cast_into(ManuallyDrop::into_inner(self.node))
    }

    #[inline(always)]
    pub(crate) unsafe fn as_branch(&self) -> &Branch<K, V, C> {
        Pointer::deref_cast(&self.node)
    }

    #[inline(always)]
    pub(crate) unsafe fn as_leaf(&self) -> &Leaf<K, V, C> {
        Pointer::deref_cast(&self.node)
    }

    #[inline(always)]
    pub(crate) unsafe fn as_branch_mut(&mut self) -> &mut Branch<K, V, C>
    where
        K: Clone,
        V: Clone,
    {
        Pointer::make_mut_cast(&mut self.node)
    }

    #[inline(always)]
    pub(crate) unsafe fn as_leaf_mut(&mut self) -> &mut Leaf<K, V, C>
    where
        K: Clone,
        V: Clone,
    {
        Pointer::make_mut_cast(&mut self.node)
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
