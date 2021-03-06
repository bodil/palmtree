use std::collections::BTreeMap;
use std::fmt::Debug;
use std::iter::FromIterator;

use crate::{config::TreeConfig, PalmTree};

#[cfg(not(test))]
use arbitrary::Arbitrary;
#[cfg(test)]
use proptest::proptest;
#[cfg(test)]
use proptest_derive::Arbitrary;

#[derive(Arbitrary, Debug)]
pub enum Construct<K, V>
where
    K: Ord,
{
    Empty,
    FromIter(BTreeMap<K, V>),
    Insert(BTreeMap<K, V>),
    Load(BTreeMap<K, V>),
}

#[derive(Arbitrary, Debug)]
pub enum Action<K, V> {
    Insert(K, V),
    Lookup(K),
    Remove(K),
    Range(Option<K>, Option<K>),
    RangeMut(Option<K>, Option<K>),
}

pub type Input<K, V> = (Construct<K, V>, Vec<Action<K, V>>);

pub fn integration_test<C>(input: Input<u8, u8>)
where
    C: TreeConfig<u8, u8>,
{
    let (constructor, actions) = input;

    let mut set: PalmTree<u8, u8, C>;
    let mut nat;

    match constructor {
        Construct::Empty => {
            set = PalmTree::new();
            nat = BTreeMap::new();
        }
        Construct::FromIter(map) => {
            nat = map.clone();
            set = PalmTree::from_iter(map.into_iter());
        }
        Construct::Insert(map) => {
            nat = map.clone();
            set = PalmTree::new();
            for (k, v) in map.into_iter() {
                set.insert(k, v);
            }
        }
        Construct::Load(map) => {
            nat = map.clone();
            set = PalmTree::load(map.into_iter());
        }
    }

    for action in actions {
        match action {
            Action::Insert(key, value) => {
                let len = nat.len() + if nat.get(&key).is_some() { 0 } else { 1 };
                nat.insert(key, value);
                set.insert(key, value);
                assert_eq!(len, set.len());
                assert_eq!(nat.len(), set.len());
            }
            Action::Lookup(key) => {
                assert_eq!(nat.get(&key), set.get(&key));
            }
            Action::Remove(key) => {
                let len = nat.len() - if nat.get(&key).is_some() { 1 } else { 0 };
                let removed_from_nat = nat.remove(&key);
                if let Some((removed_key, removed_value)) = set.remove(&key) {
                    assert_eq!(removed_key, key);
                    assert_eq!(Some(removed_value), removed_from_nat);
                }
                assert_eq!(len, set.len());
                assert_eq!(nat.len(), set.len());
            }
            Action::Range(left, right) => {
                let set_iter;
                let nat_iter;
                match (left, right) {
                    (Some(mut left), Some(mut right)) => {
                        if left > right {
                            std::mem::swap(&mut left, &mut right);
                        }
                        set_iter = set.range(left..right);
                        nat_iter = nat.range(left..right);
                    }
                    (Some(left), None) => {
                        set_iter = set.range(left..);
                        nat_iter = nat.range(left..);
                    }
                    (None, Some(right)) => {
                        set_iter = set.range(..right);
                        nat_iter = nat.range(..right);
                    }
                    (None, None) => {
                        set_iter = set.range(..);
                        nat_iter = nat.range(..);
                    }
                }
                let expected: Vec<_> = nat_iter.map(|(k, v)| (*k, *v)).collect();
                let actual: Vec<_> = set_iter.map(|(k, v)| (*k, *v)).collect();
                assert_eq!(expected, actual);
            }
            Action::RangeMut(left, right) => {
                let set_iter;
                let nat_iter;
                match (left, right) {
                    (Some(mut left), Some(mut right)) => {
                        if left > right {
                            std::mem::swap(&mut left, &mut right);
                        }
                        set_iter = set.range_mut(left..right);
                        nat_iter = nat.range_mut(left..right);
                    }
                    (Some(left), None) => {
                        set_iter = set.range_mut(left..);
                        nat_iter = nat.range_mut(left..);
                    }
                    (None, Some(right)) => {
                        set_iter = set.range_mut(..right);
                        nat_iter = nat.range_mut(..right);
                    }
                    (None, None) => {
                        set_iter = set.range_mut(..);
                        nat_iter = nat.range_mut(..);
                    }
                }
                let expected: Vec<_> = nat_iter.map(|(k, v)| (*k, *v)).collect();
                let actual: Vec<_> = set_iter.map(|(k, v)| (*k, *v)).collect();
                assert_eq!(expected, actual);
            }
        }

        // Check len()
        assert_eq!(nat.len(), set.len());

        // Immutable ref iterator
        let expected: Vec<_> = nat.iter().map(|(k, v)| (*k, *v)).collect();
        let actual: Vec<_> = set.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, actual);

        // Mutable ref iterator
        let expected: Vec<_> = nat.iter_mut().map(|(k, v)| (*k, *v)).collect();
        let actual: Vec<_> = set.iter_mut().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, actual);

        // Consuming iterator
        let expected: Vec<_> = nat.clone().into_iter().collect();
        let actual: Vec<_> = set.clone().into_iter().collect();
        assert_eq!(expected, actual);
    }
}

#[cfg(test)]
proptest! {
    #[test]
    fn integration_proptest(input: Input<u8,u8>) {
        use crate::{config::Tree64, pointer::Unique};
        integration_test::<Tree64<Unique>>(input);
    }
}
