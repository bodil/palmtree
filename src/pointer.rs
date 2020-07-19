#![allow(missing_debug_implementations)]

use std::{
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
};

pub trait PointerKind {
    unsafe fn new<A>(value: A) -> Self;
    unsafe fn into_raw<A>(self) -> NonNull<A>;
    unsafe fn from_raw<A>(ptr: NonNull<A>) -> Self;
    unsafe fn deref<A>(&self) -> &A;
    unsafe fn make_mut<A: Clone>(&mut self) -> &mut A;
    unsafe fn drop_ptr<A>(&mut self);
    unsafe fn clone<A: Clone>(&self) -> Self;
}

pub struct Unique {
    data: MaybeUninit<Box<()>>,
}

impl Unique {
    unsafe fn from_box<A>(data: Box<A>) -> Self {
        let mut out = Self {
            data: MaybeUninit::uninit(),
        };
        out.data.as_mut_ptr().cast::<Box<A>>().write(data);
        out
    }

    unsafe fn cast_into<A>(self) -> Box<A> {
        std::mem::transmute(self)
    }
}

impl PointerKind for Unique {
    unsafe fn new<A>(value: A) -> Self {
        Self::from_box(Box::new(value))
    }

    unsafe fn into_raw<A>(self) -> NonNull<A> {
        Box::leak(self.cast_into::<A>()).into()
    }

    unsafe fn from_raw<A>(mut ptr: NonNull<A>) -> Self {
        Self::from_box(Box::from_raw(ptr.as_mut()))
    }

    unsafe fn deref<A>(&self) -> &A {
        (*self.data.as_ptr().cast::<Box<A>>()).deref()
    }

    unsafe fn make_mut<A>(&mut self) -> &mut A {
        (*self.data.as_mut_ptr().cast::<Box<A>>()).deref_mut()
    }

    unsafe fn drop_ptr<A>(&mut self) {
        std::ptr::drop_in_place(self.data.as_mut_ptr().cast::<Box<A>>())
    }

    unsafe fn clone<A: Clone>(&self) -> Self {
        Self::new(self.deref::<A>().clone())
    }
}

pub struct Shared {
    data: MaybeUninit<Rc<()>>,
}

impl Shared {
    unsafe fn from_rc<A>(data: Rc<A>) -> Self {
        let mut out = Self {
            data: MaybeUninit::uninit(),
        };
        out.data.as_mut_ptr().cast::<Rc<A>>().write(data);
        out
    }

    unsafe fn cast_into<A>(self) -> Rc<A> {
        std::mem::transmute(self)
    }
}

impl PointerKind for Shared {
    unsafe fn new<A>(value: A) -> Self {
        Self::from_rc(Rc::new(value))
    }

    unsafe fn into_raw<A>(self) -> NonNull<A> {
        NonNull::new_unchecked(Rc::into_raw(self.cast_into::<A>()) as *mut A)
    }

    unsafe fn from_raw<A>(ptr: NonNull<A>) -> Self {
        Self::from_rc(Rc::from_raw(ptr.as_ptr()))
    }

    unsafe fn deref<A>(&self) -> &A {
        (*self.data.as_ptr().cast::<Rc<A>>()).deref()
    }

    unsafe fn make_mut<A: Clone>(&mut self) -> &mut A {
        Rc::make_mut(&mut *self.data.as_mut_ptr().cast::<Rc<A>>())
    }

    unsafe fn drop_ptr<A>(&mut self) {
        std::ptr::drop_in_place(self.data.as_mut_ptr().cast::<Rc<A>>())
    }

    unsafe fn clone<A: Clone>(&self) -> Self {
        Self::from_rc::<A>((&*self.data.as_ptr().cast::<Rc<A>>()).clone())
    }
}

pub struct SyncShared {
    data: MaybeUninit<Arc<()>>,
}

impl SyncShared {
    unsafe fn from_arc<A>(data: Arc<A>) -> Self {
        let mut out = Self {
            data: MaybeUninit::uninit(),
        };
        out.data.as_mut_ptr().cast::<Arc<A>>().write(data);
        out
    }

    unsafe fn cast_into<A>(self) -> Arc<A> {
        std::mem::transmute(self)
    }
}

impl PointerKind for SyncShared {
    unsafe fn new<A>(value: A) -> Self {
        Self::from_arc(Arc::new(value))
    }

    unsafe fn into_raw<A>(self) -> NonNull<A> {
        NonNull::new_unchecked(Arc::into_raw(self.cast_into::<A>()) as *mut A)
    }

    unsafe fn from_raw<A>(ptr: NonNull<A>) -> Self {
        Self::from_arc(Arc::from_raw(ptr.as_ptr()))
    }

    unsafe fn deref<A>(&self) -> &A {
        (*self.data.as_ptr().cast::<Box<A>>()).deref()
    }

    unsafe fn make_mut<A: Clone>(&mut self) -> &mut A {
        Arc::make_mut(&mut *self.data.as_mut_ptr().cast::<Arc<A>>())
    }

    unsafe fn drop_ptr<A>(&mut self) {
        std::ptr::drop_in_place(self.data.as_mut_ptr().cast::<Arc<A>>())
    }

    unsafe fn clone<A: Clone>(&self) -> Self {
        Self::from_arc::<A>((&*self.data.as_ptr().cast::<Arc<A>>()).clone())
    }
}

pub(crate) struct Pointer<A, Kind: PointerKind> {
    data: ManuallyDrop<Kind>,
    kind: PhantomData<A>,
}

unsafe impl<A, Kind> Send for Pointer<A, Kind> where Kind: PointerKind + Send {}
unsafe impl<A, Kind> Sync for Pointer<A, Kind> where Kind: PointerKind + Sync {}

impl<A, Kind: PointerKind> Pointer<A, Kind> {
    fn from_data(data: Kind) -> Self {
        Self {
            data: ManuallyDrop::new(data),
            kind: PhantomData,
        }
    }

    pub(crate) fn new(value: A) -> Self {
        Self::from_data(unsafe { Kind::new(value) })
    }

    pub(crate) fn into_raw(mut this: Self) -> NonNull<A> {
        let ptr = unsafe { ManuallyDrop::take(&mut this.data).into_raw::<A>() };
        std::mem::forget(this);
        ptr
    }

    pub(crate) unsafe fn from_raw(ptr: NonNull<A>) -> Self {
        Self::from_data(Kind::from_raw::<A>(ptr))
    }

    pub(crate) fn make_mut(this: &mut Self) -> &mut A
    where
        A: Clone,
    {
        unsafe { this.data.make_mut::<A>() }
    }

    pub(crate) unsafe fn cast_into<B>(this: Self) -> Pointer<B, Kind> {
        Pointer::from_raw(Self::into_raw(this).cast())
    }

    pub(crate) unsafe fn deref_cast<B>(this: &Self) -> &B {
        this.data.deref().deref::<B>()
    }

    pub(crate) unsafe fn make_mut_cast<B>(this: &mut Self) -> &mut B
    where
        B: Clone,
    {
        this.data.make_mut::<B>()
    }
}

impl<A, Kind> Drop for Pointer<A, Kind>
where
    Kind: PointerKind,
{
    fn drop(&mut self) {
        unsafe { self.data.drop_ptr::<A>() }
    }
}

impl<A, Kind> Deref for Pointer<A, Kind>
where
    Kind: PointerKind,
{
    type Target = A;
    fn deref(&self) -> &Self::Target {
        unsafe { self.data.deref().deref::<A>() }
    }
}

impl<A, Kind> From<A> for Pointer<A, Kind>
where
    Kind: PointerKind,
{
    fn from(value: A) -> Self {
        Self::new(value)
    }
}

impl<A, Kind> Clone for Pointer<A, Kind>
where
    A: Clone,
    Kind: PointerKind,
{
    fn clone(&self) -> Self {
        Self::from_data(unsafe { self.data.clone::<A>() })
    }
}
