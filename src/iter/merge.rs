enum Next {
    Left,
    Right,
}
use self::Next::*;
use std::fmt::{Debug, Error, Formatter};

pub struct MergeIter<A, L, R, F> {
    left: L,
    right: R,
    next_left: Option<A>,
    next_right: Option<A>,
    next: Next,
    compare: F,
}

impl<A, L, R, F> MergeIter<A, L, R, F>
where
    L: Iterator<Item = A>,
    R: Iterator<Item = A>,
    F: Fn(&A, &A) -> bool,
{
    pub fn merge(mut left: L, mut right: R, compare: F) -> Self {
        let next_left = left.next();
        let next_right = right.next();
        let next = Self::choose_next(&next_left, &next_right, &compare);
        Self {
            left,
            right,
            next_left,
            next_right,
            next,
            compare,
        }
    }

    fn choose_next(left: &Option<A>, right: &Option<A>, compare: impl Fn(&A, &A) -> bool) -> Next {
        match (left, right) {
            (Some(left), Some(right)) if compare(left, right) => Right,
            (None, Some(_)) => Right,
            _ => Left,
        }
    }
}

impl<A, L, R, F> Iterator for MergeIter<A, L, R, F>
where
    L: Iterator<Item = A>,
    R: Iterator<Item = A>,
    F: Fn(&A, &A) -> bool,
{
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        let next_result = match self.next {
            Left => std::mem::replace(&mut self.next_left, self.left.next()),
            Right => std::mem::replace(&mut self.next_right, self.right.next()),
        };
        self.next = Self::choose_next(&self.next_left, &self.next_right, &self.compare);
        next_result
    }
}

impl<A, L, R, F> Debug for MergeIter<A, L, R, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "MergeIter")
    }
}
