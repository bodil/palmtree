use crate::{branch::node::Node, PointerKind};
use generic_array::ArrayLength;
use std::marker::PhantomData;
use typenum::{IsGreater, U3, U64};

pub trait TreeConfig<K, V> {
    type BranchSize: ArrayLength<K> + ArrayLength<Node<K, V, Self>> + IsGreater<U3>;
    type LeafSize: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>;
    type PointerKind: PointerKind;
}

#[derive(Debug, Clone, Copy)]
pub struct Tree64<Kind: PointerKind>(PhantomData<Kind>);
impl<K, V, Kind: PointerKind> TreeConfig<K, V> for Tree64<Kind> {
    type BranchSize = U64;
    type LeafSize = U64;
    type PointerKind = Kind;
}
