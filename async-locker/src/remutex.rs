use std::cell::UnsafeCell;

use crate::share_lock::ShareGuard;
use crate::WakerSet;
use locker::remutex::RawReentrantMutex;

#[cfg(feature = "extra")]
pub mod simple;

#[cfg(feature = "std")]
pub mod std_thread;

#[cfg(all(feature = "extra", feature = "std"))]
pub mod global;

pub mod raw;

#[repr(C)]
pub struct ReentrantMutex<L, W, T: ?Sized> {
    raw: raw::ReentrantMutex<L, W>,
    value: UnsafeCell<T>,
}

impl<L: RawReentrantMutex + locker::Init, W: WakerSet + locker::Init, T: Default> Default
    for ReentrantMutex<L, W, T>
{
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Sync + RawReentrantMutex, W: Sync, T: Send> Sync for ReentrantMutex<L, W, T> {}

impl<L, W, T> ReentrantMutex<L, W, T> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const fn from_raw_parts(raw: raw::ReentrantMutex<L, W>, value: T) -> Self {
        Self {
            raw,
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::ReentrantMutex<L, W>, T) {
        (self.raw, self.value.into_inner())
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, W, T: ?Sized> ReentrantMutex<L, W, T> {
    #[inline]
    pub const fn raw(&self) -> &raw::ReentrantMutex<L, W> {
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
            pub unsafe fn raw_mut(&mut self) -> &mut raw::ReentrantMutex<L, W> {
                &mut self.raw
            }

            #[inline]
            pub fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        }
    }
}

impl<L: RawReentrantMutex + locker::Init, W: WakerSet + locker::Init, T> ReentrantMutex<L, W, T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new(value: T) -> Self {
                Self::from_raw_parts(locker::Init::INIT, value)
            }
        } else {
            #[inline]
            pub fn new(value: T) -> Self {
                Self::from_raw_parts(locker::Init::INIT, value)
            }
        }
    }
}

impl<L: RawReentrantMutex, W: WakerSet, T: ?Sized> ReentrantMutex<L, W, T>
where
    L::ShareGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn lock(&self) -> ShareGuard<'_, L, W, T> {
        unsafe { ShareGuard::from_raw_parts(self.raw.lock().await, self.value.get()) }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<ShareGuard<'_, L, W, T>> {
        unsafe {
            Some(ShareGuard::from_raw_parts(
                self.raw.try_lock()?,
                self.value.get(),
            ))
        }
    }
}
