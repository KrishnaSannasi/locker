//! A type-safe implementation of a `Mutex`

use crate::exclusive_lock::RawExclusiveGuard;
use crate::mutex::RawMutex;

/// A mutual exclusion primitive useful for protecting shared data
///
/// This mutex will block threads waiting for the lock to become available.
/// The mutex can also be statically initialized or created via a `from_raw` constructor.
#[repr(transparent)]
pub struct Mutex<L> {
    lock: L,
}

impl<L: RawMutex> Default for Mutex<L> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<L> Mutex<L> {
    /// Create a new raw mutex
    ///
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw(lock: L) -> Self {
        Self { lock }
    }

    /// consume the mutex and return the underlying lock
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

impl<L: RawMutex> Mutex<L> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// Create a new raw mutex
            #[inline]
            pub const fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        } else {
            /// Create a new raw mutex
            #[inline]
            pub fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        }
    }
}

impl<L: RawMutex> Mutex<L>
where
    L::ExclusiveGuardTraits: crate::Inhabitted,
{
    /// Acquires a mutex, blocking the current thread until it is able to do so.
    ///
    /// This function will block the local thread until it is available to acquire
    /// the mutex. Upon returning, the thread is the only thread with the mutex held.
    /// An RAII guard is returned to allow scoped unlock of the lock. When the guard
    /// goes out of scope, the mutex will be unlocked.
    ///
    /// Attempts to lock a `Mutex` in the thread which already holds the lock will result in a deadlock or panic.
    ///
    /// # Panic
    ///
    /// This function may panic if it is impossible to acquire the lock (in the case of deadlock or
    /// single threaded mutex)
    #[inline]
    pub fn lock(&self) -> RawExclusiveGuard<'_, L> {
        unsafe {
            self.lock.exc_lock();
            RawExclusiveGuard::from_raw(&self.lock)
        }
    }

    /// Attempts to acquire this lock.
    ///
    /// If the lock could not be acquired at this time, then None is returned.
    /// Otherwise, an RAII guard is returned. The lock will be unlocked when the guard is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_lock(&self) -> Option<RawExclusiveGuard<'_, L>> {
        unsafe {
            if self.lock.exc_try_lock() {
                Some(RawExclusiveGuard::from_raw(&self.lock))
            } else {
                None
            }
        }
    }
}
