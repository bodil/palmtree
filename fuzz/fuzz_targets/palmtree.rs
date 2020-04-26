#![no_main]

use std::collections::BTreeMap;
use std::fmt::Debug;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

use palmtree::PalmTree;

#[derive(Arbitrary, Debug)]
enum Construct<K, V>
where
    K: Ord,
{
    Empty,
    Insert(BTreeMap<K, V>),
    Load(BTreeMap<K, V>),
}

#[derive(Arbitrary, Debug)]
enum Action<K, V> {
    Insert(K, V),
    Lookup(K),
    Remove(K),
    Range(Option<K>, Option<K>),
}

#[derive(Arbitrary)]
struct Actions<K, V>(Vec<Action<K, V>>);

type Input<K, V> = (Construct<K, V>, Vec<Action<K, V>>);

fuzz_target!(|input: Input<u8, u8>| {
    let (constructor, actions) = input;

    let mut set;
    let mut nat;

    match constructor {
        Construct::Empty => {
            set = PalmTree::new();
            nat = BTreeMap::new();
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
        }
        assert_eq!(nat.len(), set.len());
        let expected: Vec<_> = nat.iter().map(|(k, v)| (*k, *v)).collect();
        let actual: Vec<_> = set.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, actual);
    }
});
