use generic_array::ArrayLength;
use std::{
    fmt::{Debug, Error, Formatter},
    mem::MaybeUninit,
};

pub(crate) struct Array<A, N>
where
    N: ArrayLength<A>,
{
    data: MaybeUninit<N::ArrayType>,
}

impl<A, N> Array<A, N>
where
    N: ArrayLength<A>,
{
    #[inline(always)]
    fn ptr(&self) -> *const A {
        self.data.as_ptr().cast()
    }

    #[inline(always)]
    fn mut_ptr(&mut self) -> *mut A {
        self.data.as_mut_ptr().cast()
    }

    #[inline(always)]
    pub(crate) unsafe fn deref(&self, length: usize) -> &[A] {
        debug_assert!(length <= N::USIZE);
        std::slice::from_raw_parts(self.ptr(), length)
    }

    #[inline(always)]
    pub(crate) unsafe fn deref_mut(&mut self, length: usize) -> &mut [A] {
        debug_assert!(length <= N::USIZE);
        std::slice::from_raw_parts_mut(self.mut_ptr(), length)
    }

    pub(crate) fn new() -> Self {
        Self {
            data: MaybeUninit::uninit(),
        }
    }

    pub(crate) unsafe fn drop(&mut self, length: usize) {
        std::ptr::drop_in_place(self.deref_mut(length))
    }

    pub(crate) unsafe fn unit(value: A) -> Self {
        let mut out = Self::new();
        out.mut_ptr().write(value);
        out
    }

    pub(crate) unsafe fn steal_from<N2: ArrayLength<A>>(
        other: &mut Array<A, N2>,
        length: usize,
        index: usize,
    ) -> Self {
        let new_length = length - index;
        debug_assert!(length <= N2::USIZE);
        debug_assert!(index < length);
        debug_assert!(new_length <= N::USIZE);
        let mut out = Self::new();
        out.mut_ptr()
            .copy_from_nonoverlapping(other.mut_ptr().add(index), new_length);
        out
    }

    pub(crate) unsafe fn clone(&self, length: usize) -> Self
    where
        A: Clone,
    {
        debug_assert!(length <= N::USIZE);
        let mut out = Self::new();
        for (index, element) in self.deref(length).iter().enumerate() {
            out.mut_ptr().add(index).write(element.clone());
        }
        out
    }

    pub(crate) unsafe fn clone_with<F>(&self, length: usize, f: F) -> Self
    where
        F: Fn(&A) -> A,
    {
        debug_assert!(length <= N::USIZE);
        let mut out = Self::new();
        for (index, element) in self.deref(length).iter().enumerate() {
            out.mut_ptr().add(index).write(f(element));
        }
        out
    }

    pub(crate) unsafe fn push(&mut self, length: usize, value: A) {
        debug_assert!(length < N::USIZE);
        self.mut_ptr().add(length).write(value);
    }

    pub(crate) unsafe fn pop(&mut self, length: usize) -> A {
        debug_assert!(length <= N::USIZE);
        debug_assert!(length > 0);
        self.mut_ptr().add(length - 1).read()
    }

    pub(crate) unsafe fn insert(&mut self, length: usize, index: usize, value: A) {
        debug_assert!(length < N::USIZE);
        debug_assert!(index <= length);
        if index < length {
            self.mut_ptr()
                .add(index)
                .copy_to(self.mut_ptr().add(index + 1), length - index);
        }
        self.mut_ptr().add(index).write(value);
    }

    pub(crate) unsafe fn insert_pair(&mut self, length: usize, index: usize, left: A, right: A) {
        debug_assert!(length < (N::USIZE - 1));
        debug_assert!(index <= length);
        if index < length {
            self.mut_ptr()
                .add(index)
                .copy_to(self.mut_ptr().add(index + 2), length - index);
        }
        self.mut_ptr().add(index).write(left);
        self.mut_ptr().add(index + 1).write(right);
    }

    pub(crate) unsafe fn remove(&mut self, length: usize, index: usize) -> A {
        debug_assert!(length <= N::USIZE);
        debug_assert!(length > 0);
        debug_assert!(index < length);
        let result = self.mut_ptr().add(index).read();
        if index + 1 < length {
            self.mut_ptr()
                .add(index + 1)
                .copy_to(self.mut_ptr().add(index), length - (index + 1));
        }
        result
    }
}

impl<A, N> Debug for Array<A, N>
where
    N: ArrayLength<A>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "Array[{}; {}]", std::any::type_name::<A>(), N::USIZE)
    }
}
