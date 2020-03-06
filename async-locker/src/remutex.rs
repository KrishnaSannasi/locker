use std::cell::UnsafeCell;

use crate::share_lock::ShareGuard;
use locker::remutex::RawReentrantMutex;

#[cfg(feature = "extra")]
pub mod simple;

#[cfg(feature = "std")]
pub mod std_thread;

#[cfg(all(feature = "extra", feature = "std"))]
pub mod global;

pub mod raw;

#[repr(C)]
pub struct ReentrantMutex<L, T: ?Sized> {
    raw: raw::ReentrantMutex<L>,
    value: UnsafeCell<T>,
}

impl<L: RawReentrantMutex, T: Default> Default for ReentrantMutex<L, T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Sync + RawReentrantMutex, T: Send> Sync for ReentrantMutex<L, T> {}

impl<L, T> ReentrantMutex<L, T> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const fn from_raw_parts(raw: raw::ReentrantMutex<L>, value: T) -> Self {
        Self {
            raw,
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::ReentrantMutex<L>, T) {
        (self.raw, self.value.into_inner())
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, T: ?Sized> ReentrantMutex<L, T> {
    #[inline]
    pub const fn raw(&self) -> &raw::ReentrantMutex<L> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn raw_mut(&mut self) -> &mut raw::ReentrantMutex<L> {
                &mut self.raw
            }

            #[inline]
            pub const fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        } else {
            #[inline]
            pub unsafe fn raw_mut(&mut self) -> &mut raw::ReentrantMutex<L> {
                &mut self.raw
            }

            #[inline]
            pub fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        }
    }
}

impl<L: RawReentrantMutex, T> ReentrantMutex<L, T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new(value: T) -> Self {
                Self::from_raw_parts(raw::ReentrantMutex::new(), value)
            }
        } else {
            #[inline]
            pub fn new(value: T) -> Self {
                Self::from_raw_parts(raw::ReentrantMutex::new(), value)
            }
        }
    }
}

impl<L: RawReentrantMutex, T: ?Sized> ReentrantMutex<L, T>
where
    L::ShareGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn lock(&self) -> ShareGuard<'_, L, T> {
        unsafe { ShareGuard::from_raw_parts(self.raw.lock().await, self.value.get()) }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<ShareGuard<'_, L, T>> {
        unsafe {
            Some(ShareGuard::from_raw_parts(
                self.raw.try_lock()?,
                self.value.get(),
            ))
        }
    }
}
