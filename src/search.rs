use sized_chunks::{types::ChunkLength, Chunk};

/// Find 'key' in 'keys', or the closest higher value.
///
/// If every value in `keys` is lower than `key`, `None` will be returned.
///
/// This is a checked version of `find_key_or_next`. No assumption about
/// the content of `keys` is needed, and it will never panic.
pub(crate) fn find_key<K, S>(keys: &Chunk<K, S>, key: &K) -> Option<usize>
where
    K: Ord,
    S: ChunkLength<K>,
{
    let size = keys.len();
    if size == 0 {
        return None;
    }

    let mut low = 0;
    let mut high = size - 1;
    while low != high {
        let mid = (low + high) / 2;
        if &keys[mid] < key {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    if low == size || &keys[low] < key {
        None
    } else {
        Some(low)
    }
}

pub(crate) fn find_key_linear<K, S>(keys: &Chunk<K, S>, target: &K) -> Option<usize>
where
    K: Ord,
    S: ChunkLength<K>,
{
    for (index, key) in keys.iter().enumerate() {
        if target <= key {
            return Some(index);
        }
    }
    None
}

/// Find `key` in `keys`, or the closest higher value.
///
/// This function assumes the highest value in `keys` is
/// not lower than `key`, and that `keys` is not empty.
///
/// If `key` is higher than the highest value in `keys`, the
/// index of the highest value will be returned.
///
/// If `keys` is empty, this function will panic.
pub(crate) fn find_key_or_next<K, S>(keys: &Chunk<K, S>, key: &K) -> usize
where
    K: Ord,
    S: ChunkLength<K>,
{
    let size = keys.len();
    let mut low = 0;
    let mut high = size - 1;
    while low != high {
        let mid = (low + high) / 2;
        if &keys[mid] < key {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

/// Find `key` in `keys`, or the closest lower value.
///
/// Invariants as in `find_or_next` above apply, but reversed.
pub(crate) fn find_key_or_prev<K, S>(keys: &Chunk<K, S>, key: &K) -> usize
where
    K: Ord,
    S: ChunkLength<K>,
{
    let size = keys.len();
    let mut low = 0;
    let mut high = size - 1;
    while low != high {
        let mid = (low + high + 1) / 2;
        if &keys[mid] > key {
            high = mid - 1;
        } else {
            low = mid;
        }
    }
    low
}

#[cfg(test)]
mod test {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn test_find_key() {
        let keys: Chunk<usize> = Chunk::from_iter(vec![2, 4, 6, 8]);
        assert_eq!(Some(0), find_key(&keys, &0));
        assert_eq!(Some(0), find_key(&keys, &1));
        assert_eq!(Some(0), find_key(&keys, &2));
        assert_eq!(Some(1), find_key(&keys, &3));
        assert_eq!(Some(1), find_key(&keys, &4));
        assert_eq!(Some(2), find_key(&keys, &5));
        assert_eq!(Some(2), find_key(&keys, &6));
        assert_eq!(Some(3), find_key(&keys, &7));
        assert_eq!(Some(3), find_key(&keys, &8));
        assert_eq!(None, find_key(&keys, &9));
        assert_eq!(None, find_key(&keys, &10));
        assert_eq!(None, find_key(&keys, &31337));
    }

    #[test]
    fn test_find_key_or_next() {
        let keys: Chunk<usize> = Chunk::from_iter(vec![2, 4, 6, 8]);
        assert_eq!(0, find_key_or_next(&keys, &0));
        assert_eq!(0, find_key_or_next(&keys, &1));
        assert_eq!(0, find_key_or_next(&keys, &2));
        assert_eq!(1, find_key_or_next(&keys, &3));
        assert_eq!(1, find_key_or_next(&keys, &4));
        assert_eq!(2, find_key_or_next(&keys, &5));
        assert_eq!(2, find_key_or_next(&keys, &6));
        assert_eq!(3, find_key_or_next(&keys, &7));
        assert_eq!(3, find_key_or_next(&keys, &8));
    }

    #[test]
    fn test_find_key_or_prev() {
        let keys: Chunk<usize> = Chunk::from_iter(vec![2, 4, 6, 8]);
        assert_eq!(0, find_key_or_prev(&keys, &2));
        assert_eq!(0, find_key_or_prev(&keys, &3));
        assert_eq!(1, find_key_or_prev(&keys, &4));
        assert_eq!(1, find_key_or_prev(&keys, &5));
        assert_eq!(2, find_key_or_prev(&keys, &6));
        assert_eq!(2, find_key_or_prev(&keys, &7));
        assert_eq!(3, find_key_or_prev(&keys, &8));
        assert_eq!(3, find_key_or_prev(&keys, &9));
        assert_eq!(3, find_key_or_prev(&keys, &10));
    }
}
