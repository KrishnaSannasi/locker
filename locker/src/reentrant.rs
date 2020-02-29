use std::cell::UnsafeCell;
use std::num::NonZeroUsize;

use crate::share_lock::{RawShareGuard, RawShareLock, ShareGuard};

#[cfg(feature = "extra")]
pub mod simple;

#[cfg(feature = "std")]
pub mod std_thread;

#[cfg(all(feature = "extra", feature = "std"))]
pub mod global;

/// # Safety
///
/// Implementations of this trait must ensure that no two active threads share
/// the same thread ID. However the ID of a thread that has exited can be re-used
/// since that thread is no longer active.
pub unsafe trait ThreadInfo {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;

    fn id(&self) -> NonZeroUsize;
}

pub unsafe trait RawReentrantMutex: crate::RawLockInfo + RawShareLock {}
#[repr(C)]
pub struct ReentrantMutex<L, T: ?Sized> {
    lock: L,
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
    /// You must pass `RawUniueLock::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw_parts(lock: L, value: T) -> Self {
        Self {
            lock,
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (L, T) {
        (self.lock, self.value.into_inner())
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, T: ?Sized> ReentrantMutex<L, T> {
    #[inline]
    pub const fn raw(&self) -> &L {
        &self.lock
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn raw_mut(&mut self) -> &mut L {
                &mut self.lock
            }

            #[inline]
            pub const fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        } else {
            #[inline]
            pub unsafe fn raw_mut(&mut self) -> &mut L {
                &mut self.lock
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
                unsafe { Self::from_raw_parts(L::INIT, value) }
            }
        } else {
            #[inline]
            pub fn new(value: T) -> Self {
                unsafe { Self::from_raw_parts(L::INIT, value) }
            }
        }
    }
}

impl<L: RawReentrantMutex, T: ?Sized> ReentrantMutex<L, T>
where
    L::ShareGuardTraits: crate::Inhabitted,
{
    #[inline]
    pub fn lock(&self) -> ShareGuard<'_, L, T> {
        unsafe { ShareGuard::from_raw_parts(RawShareGuard::new(&self.lock), self.value.get()) }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<ShareGuard<'_, L, T>> {
        unsafe {
            Some(ShareGuard::from_raw_parts(
                RawShareGuard::try_new(&self.lock)?,
                self.value.get(),
            ))
        }
    }
}
