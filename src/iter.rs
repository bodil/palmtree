use crate::{search::PathedPointer, PalmTree};
use std::{
    cmp::Ordering,
    fmt::{Debug, Error, Formatter},
    ops::{Bound, RangeBounds},
};

pub struct PalmTreeIter<'a, K, V> {
    left: PathedPointer<'a, K, V>,
    right: PathedPointer<'a, K, V>,
}

impl<'a, K, V> Clone for PalmTreeIter<'a, K, V>
where
    K: Clone + Ord,
{
    fn clone(&self) -> Self {
        Self {
            left: self.left.clone(),
            right: self.right.clone(),
        }
    }
}

impl<'a, K, V> PalmTreeIter<'a, K, V>
where
    K: Clone + Ord,
{
    fn null() -> Self {
        Self {
            left: PathedPointer::null(),
            right: PathedPointer::null(),
        }
    }

    pub(crate) fn new<R>(tree: &'a PalmTree<K, V>, range: R) -> Self
    where
        R: RangeBounds<K>,
    {
        match (range.start_bound(), range.end_bound()) {
            (Bound::Excluded(left), Bound::Excluded(right)) if left == right => {
                panic!("PalmTreeIter: start and end bounds are equal and excluding each other")
            }
            (Bound::Included(left), Bound::Included(right))
            | (Bound::Included(left), Bound::Excluded(right))
            | (Bound::Excluded(left), Bound::Included(right))
            | (Bound::Excluded(left), Bound::Excluded(right))
                if left.cmp(right) == Ordering::Greater =>
            {
                panic!("PalmTreeIter: range start is greater than range end");
            }
            _ => {}
        }

        let left;
        let right;

        if let Some(ref tree) = tree.root {
            left = match range.start_bound() {
                Bound::Included(key) => PathedPointer::key_or_higher(tree, key),
                Bound::Excluded(key) => PathedPointer::higher_than_key(tree, key),
                Bound::Unbounded => PathedPointer::lowest(tree),
            };
            if left.is_null() {
                return Self::null();
            }

            right = match range.end_bound() {
                Bound::Included(key) => PathedPointer::key_or_lower(tree, key),
                Bound::Excluded(key) => PathedPointer::lower_than_key(tree, key),
                Bound::Unbounded => PathedPointer::highest(tree),
            };
            if right.is_null() {
                return Self::null();
            }

            Self { left, right }
        } else {
            // Tree has no root, iterator is empty.
            Self::null()
        }
    }

    fn step_forward(&mut self) {
        let result = self.left.step_forward();
        debug_assert!(result);
    }

    fn step_back(&mut self) {
        let result = self.right.step_back();
        debug_assert!(result);
    }
}

impl<'a, K, V> Iterator for PalmTreeIter<'a, K, V>
where
    K: Clone + Ord,
{
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let left_key = self.left.key()?;
        let right_key = self.right.key()?;
        // If left key is greather than right key, we're done.
        let cmp = left_key.cmp(right_key);
        if cmp == Ordering::Greater {
            self.left.clear();
            self.right.clear();
            return None;
        }
        let value = self.left.value().unwrap();
        if cmp == Ordering::Equal {
            self.left.clear();
            self.right.clear();
        } else {
            self.step_forward();
        }
        Some((left_key, value))
    }
}

impl<'a, K, V> DoubleEndedIterator for PalmTreeIter<'a, K, V>
where
    K: Clone + Ord,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let left_key = self.left.key()?;
        let right_key = self.right.key()?;
        // If left key is greather than right key, we're done.
        let cmp = left_key.cmp(right_key);
        if cmp == Ordering::Greater {
            self.left.clear();
            self.right.clear();
            return None;
        }
        let value = self.right.value().unwrap();
        if cmp == Ordering::Equal {
            self.left.clear();
            self.right.clear();
        } else {
            self.step_back();
        }
        Some((right_key, value))
    }
}

impl<'a, K, V> Debug for PalmTreeIter<'a, K, V>
where
    K: Clone + Ord + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_map().entries(self.clone()).finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn iterate_single_leaf() {
        let size = 64usize;
        let tree = PalmTree::load((0..size).map(|i| (i, i)));
        // let expected: Vec<_> = (0..size).map(|i| (i, i)).collect();
        // let result: Vec<_> = tree.iter().map(|(k, v)| (*k, *v)).collect();
        tree.iter().for_each(|i| {
            criterion::black_box(i);
        });
        // assert_eq!(expected, result);
    }

    #[test]
    fn iterate_forward() {
        let size = 65536usize;
        let tree = PalmTree::load((0..size).map(|i| (i, i)));
        let expected: Vec<_> = (0..size).map(|i| (i, i)).collect();
        let result: Vec<_> = tree.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn iterate_backward() {
        let size = 65536usize;
        let tree = PalmTree::load((0..size).map(|i| (i, i)));
        let expected: Vec<_> = (0..size).map(|i| (i, i)).rev().collect();
        let result: Vec<_> = tree.iter().map(|(k, v)| (*k, *v)).rev().collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn empty_range_iter() {
        let tree = PalmTree::load((0..1usize).map(|i| (i, i)));
        let expected = Vec::<(usize, usize)>::new();
        let result: Vec<_> = tree.range(0..0).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn wide_end_range_iter() {
        let tree = PalmTree::load((0..1usize).map(|i| (i, i)));
        let expected = vec![(0usize, 0usize)];
        let result: Vec<_> = tree.range(0..255).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn wide_start_range_iter() {
        let tree = PalmTree::load((0..1usize).map(|i| (i, i)));
        let expected: Vec<(usize, usize)> = vec![];
        let result: Vec<_> = tree.range(100..).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    #[should_panic]
    fn descending_range_iter() {
        let tree = PalmTree::load((0..1usize).map(|i| (i, i)));
        let expected = Vec::<(usize, usize)>::new();
        let result: Vec<_> = tree.range(255..0).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn end_before_first_key_iter() {
        let tree = PalmTree::load((1..2usize).map(|i| (i, i)));
        let expected: Vec<(usize, usize)> = vec![];
        let result: Vec<_> = tree.range(..0).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn start_after_last_key_iter() {
        let tree = PalmTree::load((1..2usize).map(|i| (i, i)));
        let expected: Vec<(usize, usize)> = vec![];
        let result: Vec<_> = tree.range(3..).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn end_before_last_key_iter() {
        let tree = PalmTree::load((0..2usize).map(|i| (i, i)));
        let expected: Vec<(usize, usize)> = vec![(0, 0)];
        let result: Vec<_> = tree.range(..=0).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn range_with_deleted_max() {
        let mut tree: PalmTree<u8, u8> = PalmTree::new();
        tree.insert(0, 0);
        tree.insert(1, 136);
        tree.remove(&1);

        // println!("{:?}", tree);

        let result: Vec<(u8, u8)> = tree.range(1..2).map(|(k, v)| (*k, *v)).collect();
        let expected: Vec<(u8, u8)> = vec![];
        assert_eq!(expected, result);
    }

    #[test]
    fn iterate_over_emptied_tree() {
        let mut tree: PalmTree<u8, u8> = PalmTree::new();
        tree.insert(0, 0);
        tree.remove(&0);
        let result: Vec<(u8, u8)> = tree.iter().map(|(k, v)| (*k, *v)).collect();
        let expected: Vec<(u8, u8)> = vec![];
        assert_eq!(expected, result);
    }

    #[test]
    fn closing_bound_lies_past_target_leaf() {
        // This test has two leaves, and the closing bound for the iterator lies exactly between them.
        // Left leaf has max key 251, right leaf has min key 254, bound is 253.
        let input = vec![
            (0, 171),
            (1, 248),
            (5, 189),
            (7, 122),
            (8, 189),
            (9, 11),
            (10, 165),
            (11, 215),
            (13, 243),
            (15, 0),
            (17, 0),
            (21, 245),
            (24, 5),
            (30, 0),
            (31, 255),
            (32, 10),
            (35, 0),
            (41, 255),
            (52, 82),
            (54, 28),
            (58, 0),
            (59, 255),
            (61, 11),
            (64, 238),
            (78, 59),
            (80, 255),
            (82, 82),
            (85, 238),
            (91, 91),
            (93, 243),
            (104, 115),
            (115, 115),
            (121, 121),
            (122, 255),
            (124, 10),
            (126, 251),
            (127, 85),
            (131, 131),
            (133, 115),
            (135, 0),
            (138, 126),
            (142, 238),
            (148, 158),
            (152, 242),
            (158, 138),
            (164, 0),
            (166, 164),
            (170, 170),
            (177, 78),
            (184, 17),
            (189, 255),
            (202, 54),
            (213, 215),
            (215, 50),
            (219, 255),
            (227, 164),
            (238, 246),
            (242, 18),
            (243, 242),
            (245, 243),
            (246, 127),
            (248, 170),
            (249, 255),
            (251, 184),
            (254, 242),
            (255, 54),
        ];
        let tree: PalmTree<u8, u8> = PalmTree::load(input.clone().into_iter());

        // println!("{:?}", tree);

        let result: Vec<(u8, u8)> = tree.range(..253).map(|(k, v)| (*k, *v)).collect();
        let expected: Vec<(u8, u8)> = input
            .clone()
            .into_iter()
            .filter(|(k, _)| k < &253)
            .collect();
        assert_eq!(expected, result);
    }
}
