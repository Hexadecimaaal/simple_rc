#![no_std]
#![feature(
    non_null_convenience,
    allocator_api,
    negative_impls,
    unsize,
    coerce_unsized,
    dispatch_from_dyn,
    dropck_eyepatch,
    cell_update
)]
extern crate alloc;

use core::{
    alloc::{AllocError, Allocator, Layout},
    cell::Cell,
    marker::Unsize,
    mem::MaybeUninit,
    ops::{CoerceUnsized, Deref, DispatchFromDyn},
    ptr::NonNull,
};

use alloc::{alloc::Global, boxed::Box};

#[repr(C)]
struct RcBox<T: ?Sized> {
    ref_count: Cell<usize>,
    value: T,
}

pub struct Rc<T: ?Sized, A: Allocator = Global> {
    ptr: NonNull<RcBox<T>>,
    alloc: A,
}

impl<T: ?Sized, A: Allocator> !Send for Rc<T, A> {}
impl<T: ?Sized, A: Allocator> !Sync for Rc<T, A> {}
impl<T: ?Sized + Unsize<U>, U: ?Sized, A: Allocator> CoerceUnsized<Rc<U, A>> for Rc<T, A> {}
impl<T: ?Sized + Unsize<U>, U: ?Sized> DispatchFromDyn<Rc<U>> for Rc<T> {}

impl<T: ?Sized, A: Allocator> Rc<T, A> {
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn inner(this: &Self) -> &RcBox<T> {
        unsafe { this.ptr.as_ref() }
    }
    #[inline]
    unsafe fn from_inner_in(ptr: NonNull<RcBox<T>>, alloc: A) -> Self {
        Self { ptr, alloc }
    }
    // #[inline]
    // unsafe fn from_ptr_in(ptr: *mut RcBox<T>, alloc: A) -> Self {
    //     Self::from_inner_in(NonNull::new_unchecked(ptr), alloc)
    // }
}

impl<T: ?Sized> Rc<T> {
    // #[inline]
    // unsafe fn from_inner(ptr: NonNull<RcBox<T>>) -> Self {
    //     Self::from_inner_in(ptr, Global)
    // }
    // #[inline]
    // unsafe fn from_ptr(ptr: *mut RcBox<T>) -> Self {
    //     Self::from_ptr_in(ptr, Global)
    // }
}

impl<T, A: Allocator> Rc<T, A> {
    #[inline]
    pub fn try_new_in(value: T, alloc: A) -> Result<Self, AllocError> {
        let layout = Layout::new::<RcBox<T>>();
        let ptr = alloc.allocate(layout)?;
        let ptr = ptr.cast();
        unsafe {
            ptr.write(RcBox {
                ref_count: Cell::new(1),
                value,
            });
        }
        Ok(unsafe { Self::from_inner_in(ptr, alloc) })
    }
    #[inline]
    pub fn try_new_uninit_in(alloc: A) -> Result<Rc<MaybeUninit<T>, A>, AllocError> {
        let layout = Layout::new::<RcBox<MaybeUninit<T>>>();
        let ptr = alloc.allocate(layout)?;
        let ptr = ptr.cast();
        unsafe {
            ptr.write(RcBox {
                ref_count: Cell::new(1),
                value: MaybeUninit::uninit(),
            });
        }
        Ok(unsafe { Rc::from_inner_in(ptr, alloc) })
    }
    #[inline]
    pub fn try_unwrap(this: Self) -> Result<T, Self> {
        if Self::inner(&this).ref_count.get() == 1 {
            let value = unsafe { Box::from_raw(this.ptr.as_ptr()).value };
            core::mem::forget(this);
            Ok(value)
        } else {
            Err(this)
        }
    }
    #[inline]
    pub fn into_inner(this: Self) -> Option<T> {
        Self::try_unwrap(this).ok()
    }
}

impl<T> Rc<T> {
    #[inline]
    pub fn try_new(value: T) -> Result<Self, AllocError> {
        Self::try_new_in(value, Global)
    }

    #[inline]
    pub fn try_new_uninit() -> Result<Rc<MaybeUninit<T>>, AllocError> {
        Self::try_new_uninit_in(Global)
    }

    /// # Panics
    /// Panics if allocation fails
    #[inline]
    pub fn new(value: T) -> Self {
        Self::try_new(value).unwrap()
    }
    /// # Panics
    /// Panics if allocation fails
    #[inline]
    #[must_use]
    pub fn new_uninit() -> Rc<MaybeUninit<T>> {
        Self::try_new_uninit_in(Global).unwrap()
    }

    #[inline]
    #[must_use]
    pub fn get_ref_count(this: &Self) -> usize {
        Self::inner(this).ref_count.get()
    }
}

impl<T: ?Sized, A: Allocator + Clone> Clone for Rc<T, A> {
    #[inline]
    fn clone(&self) -> Self {
        Self::inner(self).ref_count.update(|x| x + 1);
        Self {
            ptr: self.ptr,
            alloc: self.alloc.clone(),
        }
    }
}

impl<T, A: Allocator> Deref for Rc<T, A> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &Self::inner(self).value
    }
}

unsafe impl<#[may_dangle] T: ?Sized, A: Allocator> Drop for Rc<T, A> {
    #[inline]
    fn drop(&mut self) {
        let ref_count = Self::inner(self).ref_count.get();
        if ref_count == 1 {
            unsafe {
                self.ptr.as_ptr().drop_in_place();
                self.alloc
                    .deallocate(self.ptr.cast(), Layout::for_value(self.ptr.as_ref()));
            }
        } else {
            Self::inner(self).ref_count.set(ref_count - 1);
        }
    }
}
