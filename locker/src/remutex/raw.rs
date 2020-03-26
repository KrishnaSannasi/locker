//! a raw reentrant mutex

use super::RawReentrantMutex;
use crate::share_lock::{RawShareGuard, RawShareLockTimed};

/// A mutual exclusion primitive useful for protecting shared data
///
/// This reentrant mutex will block threads waiting for the lock to become available.
/// The reentrant mutex can also be statically initialized or created via the `from_raw` constructor.
///
/// Each lock can only be used in a single thread, and the reentrant mutex can be locked as many times
/// within a single thread. But two threads can't both hold a lock into the reentrant mutex as the same time.
#[repr(C)]
pub struct ReentrantMutex<L> {
    lock: L,
}

impl<L: RawReentrantMutex> Default for ReentrantMutex<L> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<L> ReentrantMutex<L> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw(lock: L) -> Self {
        Self { lock }
    }

    /// Consumes this reentrant mutex, returning the underlying lock
    #[inline]
    pub fn into_inner(self) -> L {
        self.lock
    }

    /// the underlying lock
    #[inline]
    pub const fn inner(&self) -> &L {
        &self.lock
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// the underlying lock
            ///
            /// # Safety
            ///
            /// You must not overwrite the underlying lock
            #[inline]
            pub const unsafe fn inner_mut(&mut self) -> &mut L {
                &mut self.lock
            }
        } else {
            /// the underlying lock
            ///
            /// # Safety
            ///
            /// You must not overwrite the underlying lock
            #[inline]
            pub unsafe fn inner_mut(&mut self) -> &mut L {
                &mut self.lock
            }
        }
    }
}

impl<L: RawReentrantMutex> ReentrantMutex<L> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// Create a new raw reentrant mutex
            #[inline]
            pub const fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        } else {
            /// Create a new raw reentrant mutex
            #[inline]
            pub fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        }
    }
}

impl<L: RawReentrantMutex> ReentrantMutex<L>
where
    L::ShareGuardTraits: crate::Inhabitted,
{
    #[inline]
    unsafe fn lock_unchecked(&self) -> RawShareGuard<'_, L> {
        RawShareGuard::from_raw(&self.lock)
    }

    /// Acquires a lock, blocking the current thread until it is able to do so.
    ///
    /// This function will block the current thread until it is available to acquire
    /// the mutex. Upon returning, the thread is the only thread with the mutex held.
    /// An RAII guard is returned to allow scoped unlock of the lock. When the guard
    /// goes out of scope, the mutex will be unlocked.
    ///
    /// If there is already a lock acquired in the current thread, then this function is non-blocking
    /// and is guaranteed to acquire the lock.
    ///
    /// # Panic
    ///
    /// This function may panic if it is impossible to acquire the lock (in the case of deadlock)
    #[inline]
    pub fn lock(&self) -> RawShareGuard<'_, L> {
        unsafe {
            self.lock.shr_lock();
            self.lock_unchecked()
        }
    }

    /// Attempts to acquire this lock.
    ///
    /// If the lock could not be acquired at this time, then None is returned.
    /// Otherwise, an RAII guard is returned. The lock will be unlocked when the guard is dropped.
    ///
    /// If there is already a lock acquired in the current thread, then this function is non-blocking
    /// and is guaranteed to acquire the lock.
    #[inline]
    pub fn try_lock(&self) -> Option<RawShareGuard<'_, L>> {
        if self.lock.shr_try_lock() {
            unsafe { Some(self.lock_unchecked()) }
        } else {
            None
        }
    }
}

impl<L: RawReentrantMutex + RawShareLockTimed> ReentrantMutex<L>
where
    L::ShareGuardTraits: crate::Inhabitted,
{
    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    ///
    /// If there is already a lock acquired in the current thread, then this function is non-blocking
    /// and is guaranteed to acquire the lock.
    #[inline]
    pub fn try_lock_until(&self, instant: L::Instant) -> Option<RawShareGuard<'_, L>> {
        if self.lock.shr_try_lock_until(instant) {
            unsafe { Some(self.lock_unchecked()) }
        } else {
            None
        }
    }

    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    ///
    /// If there is already a lock acquired in the current thread, then this function is non-blocking
    /// and is guaranteed to acquire the lock.
    #[inline]
    pub fn try_lock_for(&self, duration: L::Duration) -> Option<RawShareGuard<'_, L>> {
        if self.lock.shr_try_lock_for(duration) {
            unsafe { Some(self.lock_unchecked()) }
        } else {
            None
        }
    }
}
