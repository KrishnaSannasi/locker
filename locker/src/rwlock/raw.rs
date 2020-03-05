//! A type-safe implementation of a `RwLock`

use super::RawRwLock;
use crate::exclusive_lock::RawExclusiveGuard;
use crate::share_lock::RawShareGuard;

/// A read-write syncronization primitive useful for protecting shared data
///
/// This rwlock will block threads waiting for the lock to become available.
/// The rwlock can also be statically initialized or created via a `from_raw` constructor.
#[repr(transparent)]
pub struct RwLock<L> {
    lock: L,
}

impl<L: RawRwLock> Default for RwLock<L> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<L> RwLock<L> {
    /// Create a new raw mutex
    ///
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw(lock: L) -> Self {
        Self { lock }
    }

    /// consume the rwlock and return the underlying lock
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

impl<L: RawRwLock> RwLock<L> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// Create a new raw rwlock
            #[inline]
            pub const fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        } else {
            #[inline]
            /// Create a new raw rwlock
            pub fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        }
    }
}

impl<L: RawRwLock> RwLock<L>
where
    L::ExclusiveGuardTraits: crate::Inhabitted,
    L::ShareGuardTraits: crate::Inhabitted,
{
    /// Locks this `RwLock` with exclusive write access, blocking the current thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers currently have access to the lock.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock` when dropped.
    ///
    /// Attempts to lock a `RwLock` in the thread which already holds the lock will result in a deadlock or panic.
    ///
    /// # Panic
    ///
    /// This function may panic if it is impossible to acquire the lock (in the case of deadlock or
    /// single threaded rwlock)
    #[inline]
    pub fn write(&self) -> RawExclusiveGuard<'_, L> {
        unsafe {
            self.lock.exc_lock();
            RawExclusiveGuard::from_raw(&self.lock)
        }
    }

    /// Attempts to lock this RwLock with exclusive write access.
    ///
    /// If the lock could not be acquired at this time, then None is returned.
    /// Otherwise, an RAII guard is returned which will release the lock when it is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_write(&self) -> Option<RawExclusiveGuard<'_, L>> {
        unsafe {
            if self.lock.exc_try_lock() {
                Some(RawExclusiveGuard::from_raw(&self.lock))
            } else {
                None
            }
        }
    }

    /// Locks this `RwLock` with shared read access, blocking the current thread until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which hold the lock.
    /// There may be other readers currently inside the lock when this method returns.
    ///
    /// Note that attempts to recursively acquire a read lock on a `RwLock` when the current thread
    /// already holds one may result in a deadlock or panic.
    ///
    /// Returns an RAII guard which will release this thread's shared access once it is dropped.
    #[inline]
    pub fn read(&self) -> RawShareGuard<'_, L> {
        unsafe {
            self.lock.shr_lock();
            RawShareGuard::from_raw(&self.lock)
        }
    }

    /// Attempts to acquire this RwLock with shared read access.
    ///
    /// If the access could not be granted at this time, then None is returned.
    /// Otherwise, an RAII guard is returned which will release the shared access when it is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_read(&self) -> Option<RawShareGuard<'_, L>> {
        unsafe {
            if self.lock.shr_try_lock() {
                Some(RawShareGuard::from_raw(&self.lock))
            } else {
                None
            }
        }
    }
}
