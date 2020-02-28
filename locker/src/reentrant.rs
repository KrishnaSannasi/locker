use std::cell::UnsafeCell;
use std::num::NonZeroUsize;

use crate::share_lock::{RawShareLock, RawShareLockExt, ShareGuard};

pub mod simple;

#[cfg(feature = "std")]
pub mod std_thread;

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

pub unsafe trait RawReentrantMutex: crate::RawLockInfo + RawShareLock + RawShareLockExt {}
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
    pub unsafe fn raw(&self) -> &L {
        &self.lock
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

impl<L: RawReentrantMutex, T> ReentrantMutex<L, T> {
    #[inline]
    pub fn new(value: T) -> Self {
        unsafe { Self::from_raw_parts(L::INIT, value) }
    }
}

impl<L: RawReentrantMutex, T: ?Sized> ReentrantMutex<L, T> {
    #[inline]
    pub fn lock(&self) -> ShareGuard<'_, L, T> {
        ShareGuard::new(self.lock.raw_shr_lock(), unsafe { &*self.value.get() })
    }

    #[inline]
    pub fn try_lock(&self) -> Option<ShareGuard<'_, L, T>> {
        Some(ShareGuard::new(self.lock.try_raw_shr_lock()?, unsafe {
            &*self.value.get()
        }))
    }
}
