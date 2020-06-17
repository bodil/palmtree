use typenum::consts::*;

pub(crate) type NodeSize = U64;
pub(crate) type LeafSize = U64;
pub(crate) type MaxHeight = U16;

#[derive(Debug)]
pub(crate) enum InsertResult<K, V> {
    // The item was added.
    Added,
    // The item replaced this value.
    Replaced(V),
    // The node was full and could not accept the item.
    Full(K, V),
}

#[derive(Debug)]
pub(crate) enum RemoveResult<K, V> {
    // The item was deleted.
    Deleted(K, V),
    // The item was deleted and its leaf is now empty.
    DeletedAndEmpty(K, V),
    // The item was not found.
    NotHere,
}
