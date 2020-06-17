enum Next {
    Left,
    Right,
}
use self::Next::*;
use std::fmt::{Debug, Error, Formatter};

pub struct MergeIter<A, L, R, Cmp, Eq> {
    left: L,
    right: R,
    next_left: Option<A>,
    next_right: Option<A>,
    next: Next,
    compare: Cmp,
    equal: Eq,
}

impl<A, L, R, Cmp, Eq> MergeIter<A, L, R, Cmp, Eq>
where
    L: Iterator<Item = A>,
    R: Iterator<Item = A>,
    Cmp: Fn(&A, &A) -> bool,
    Eq: Fn(&A, &A) -> bool,
{
    pub fn merge(mut left: L, mut right: R, compare: Cmp, equal: Eq) -> Self {
        let next_left = left.next();
        let next_right = right.next();
        let next = Self::choose_next(&next_left, &next_right, &compare);
        let mut out = Self {
            left,
            right,
            next_left,
            next_right,
            next,
            compare,
            equal,
        };
        out.check_eq();
        out
    }

    fn choose_next(left: &Option<A>, right: &Option<A>, compare: impl Fn(&A, &A) -> bool) -> Next {
        match (left, right) {
            (Some(left), Some(right)) if compare(left, right) => Right,
            (None, Some(_)) => Right,
            _ => Left,
        }
    }

    fn check_eq(&mut self) {
        if let (Some(left), Some(right)) = (&self.next_left, &self.next_right) {
            if (self.equal)(left, right) {
                match self.next {
                    Left => self.next_right = self.right.next(),
                    Right => self.next_left = self.left.next(),
                }
            }
        }
    }
}

impl<A, L, R, Cmp, Eq> Iterator for MergeIter<A, L, R, Cmp, Eq>
where
    L: Iterator<Item = A>,
    R: Iterator<Item = A>,
    Cmp: Fn(&A, &A) -> bool,
    Eq: Fn(&A, &A) -> bool,
{
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        let next_result = match self.next {
            Left => std::mem::replace(&mut self.next_left, self.left.next()),
            Right => std::mem::replace(&mut self.next_right, self.right.next()),
        };
        self.next = Self::choose_next(&self.next_left, &self.next_right, &self.compare);
        self.check_eq();
        next_result
    }
}

impl<A, L, R, Cmp, Eq> Debug for MergeIter<A, L, R, Cmp, Eq> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "MergeIter")
    }
}
