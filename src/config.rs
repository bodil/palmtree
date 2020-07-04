use crate::branch::node::Node;
use generic_array::ArrayLength;
use typenum::{IsGreater, U3, U64};

pub trait TreeConfig<K, V> {
    type BranchSize: ArrayLength<K> + ArrayLength<Node<K, V, Self>> + IsGreater<U3>;
    type LeafSize: ArrayLength<K> + ArrayLength<V> + IsGreater<U3>;
}

#[derive(Debug, Clone, Copy)]
pub struct Tree64;
impl<K, V> TreeConfig<K, V> for Tree64 {
    type BranchSize = U64;
    type LeafSize = U64;
}
