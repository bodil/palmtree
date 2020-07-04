use crate::{branch::Branch, config::TreeConfig, leaf::Leaf, search::PathedPointer, PalmTree};
use std::fmt::{Debug, Error, Formatter};

#[derive(Debug)]
pub enum Entry<'a, K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    Vacant(VacantEntry<'a, K, V, C>),
    Occupied(OccupiedEntry<'a, K, V, C>),
}

impl<'a, K, V, C> Entry<'a, K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    #[inline(always)]
    pub(crate) fn new(tree: &'a mut PalmTree<K, V, C>, key: K) -> Self {
        if let Some(ref mut root) = tree.root {
            match PathedPointer::exact_key(root, &key) {
                Ok(cursor) => Self::Occupied(OccupiedEntry { tree, cursor }),
                Err(cursor) => Self::Vacant(VacantEntry { key, tree, cursor }),
            }
        } else {
            Self::Vacant(VacantEntry {
                key,
                tree,
                cursor: PathedPointer::null(),
            })
        }
    }
}

// Vacant entry

pub struct VacantEntry<'a, K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    tree: &'a mut PalmTree<K, V, C>,
    cursor: PathedPointer<&'a mut (K, V), K, V, C>,
    key: K,
}

impl<'a, K, V, C> VacantEntry<'a, K, V, C>
where
    K: 'a + Ord + Clone,
    V: 'a,
    C: TreeConfig<K, V>,
{
    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn into_key(self) -> K {
        self.key
    }

    pub fn insert(mut self, value: V) -> &'a mut V {
        // If the tree is empty, just insert a new node.
        // Note that the tree could have an allocated root even when empty,
        // and we're just ignoring that here on the assumption that it's better
        // to avoid an extra null check on every insert than optimise for an infrequent use case.
        if self.tree.is_empty() {
            self.tree.root = Some(Branch::unit(Leaf::unit(self.key, value).into()).into());
            self.tree.size = 1;
            return &mut self
                .tree
                .root
                .as_mut()
                .unwrap()
                .get_leaf_mut(0)
                .values_mut()[0];
        }
        let result = if self.cursor.is_null() {
            unsafe {
                self.cursor
                    .push_last(self.tree.root.as_mut().unwrap(), self.key, value)
            }
        } else {
            unsafe { self.cursor.insert(self.key, value) }
        };
        let ptr: *mut V = match result {
            Ok(mut ptr) => {
                self.tree.size += 1;
                unsafe { ptr.value_mut().unwrap() }
            }
            Err((key, value)) => {
                let root = self.tree.root.as_mut().unwrap();
                PalmTree::split_root(root);
                self.cursor = PathedPointer::exact_key(root, &key).unwrap_err();
                self.key = key;
                self.insert(value)
            }
        };
        unsafe { &mut *ptr }
    }
}

impl<'a, K, V, C> Debug for VacantEntry<'a, K, V, C>
where
    K: Ord + Clone + Debug,
    V: Debug,
    C: TreeConfig<K, V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "VacantEntry({:?})", self.key())
    }
}

// Occupied entry

pub struct OccupiedEntry<'a, K, V, C>
where
    K: Ord + Clone,
    C: TreeConfig<K, V>,
{
    tree: &'a mut PalmTree<K, V, C>,
    cursor: PathedPointer<&'a mut (K, V), K, V, C>,
}

impl<'a, K, V, C> OccupiedEntry<'a, K, V, C>
where
    K: 'a + Ord + Clone,
    V: 'a,
    C: TreeConfig<K, V>,
{
    pub fn key(&self) -> &K {
        unsafe { self.cursor.key() }.unwrap()
    }

    pub fn get(&self) -> &V {
        unsafe { self.cursor.value() }.unwrap()
    }

    pub fn get_mut(&mut self) -> &mut V {
        unsafe { self.cursor.value_mut() }.unwrap()
    }

    pub fn insert(&mut self, value: V) -> V {
        std::mem::replace(self.get_mut(), value)
    }

    pub fn remove_entry(self) -> (K, V) {
        self.tree.size -= 1;
        unsafe { self.cursor.remove() }
    }

    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    pub fn into_mut(self) -> &'a mut V {
        unsafe { self.cursor.into_entry_mut() }.1
    }
}

impl<'a, K, V, C> Debug for OccupiedEntry<'a, K, V, C>
where
    K: Ord + Clone + Debug,
    V: Debug,
    C: TreeConfig<K, V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "OccupiedEntry({:?} => {:?})", self.key(), self.get())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::StdPalmTree;
    use std::iter::FromIterator;

    #[test]
    fn insert_with_entry() {
        let mut tree: StdPalmTree<usize, usize> = PalmTree::new();
        let size = 131_072;
        for i in 0..size {
            match tree.entry(i) {
                Entry::Vacant(entry) => {
                    entry.insert(i);
                }
                Entry::Occupied(_) => {
                    panic!("found an occupied entry where none should be at {}", i);
                }
            }
        }
        for i in 0..size {
            assert_eq!(Some(&i), tree.get(&i));
        }
    }

    #[test]
    fn delete_with_entry() {
        let size = 131_072;
        let mut tree: StdPalmTree<usize, usize> = PalmTree::from_iter((0..size).map(|i| (i, i)));
        for i in 0..size {
            match tree.entry(i) {
                Entry::Vacant(_entry) => {
                    panic!("unexpected vacant entry at {}", i);
                }
                Entry::Occupied(entry) => {
                    assert_eq!(entry.remove(), i);
                }
            }
        }
        assert_eq!(0, tree.len());
    }
}
