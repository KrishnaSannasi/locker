use super::{RawShareLock, RawShareLockFair};
use crate::{Inhabitted, RawLockInfo};

/// A RAII implementation of a scoped shared lock
///
/// This type represents a *shr lock*, and while it is alive there is an active *shr lock*
///
/// Once this structure is dropped, that *shr lock* will automatically be released by calling
/// [`RawShareLock::shr_unlock`]. If you want to release the *shr lock* using a fair unlock
/// protocol, use [`RawShareGuard::unlock_fair`](crate::share_lock::RawShareGuard#method.unlock_fair)
pub type RawShareGuard<'a, L> = _RawShareGuard<'a, L, <L as RawLockInfo>::ShareGuardTraits>;

#[doc(hidden)]
pub struct _RawShareGuard<'a, L: RawShareLock, Tr> {
    lock: &'a L,
    _traits: Tr,
}

impl<'a, L: RawShareLock, Tr> Drop for _RawShareGuard<'_, L, Tr> {
    fn drop(&mut self) {
        unsafe { self.lock.shr_unlock() }
    }
}

impl<'a, L: RawShareLock + RawLockInfo> RawShareGuard<'a, L>
where
    L::ShareGuardTraits: Inhabitted,
{
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// # Safety
            ///
            /// A *shr lock* must owned for the given `lock`
            pub const unsafe fn from_raw(lock: &'a L) -> Self {
                Self { lock, _traits: Inhabitted::INIT }
            }
        } else {
            /// # Safety
            ///
            /// A *shr lock* must owned for the given `lock`
            pub unsafe fn from_raw(lock: &'a L) -> Self {
                Self { lock, _traits: Inhabitted::INIT }
            }
        }
    }

    /// Create a new `RawShareGuard`
    ///
    /// blocks until lock is acquired
    ///
    /// # Panic
    ///
    /// This function may panic if the lock is cannot be acquired
    pub fn new(lock: &'a L) -> Self {
        lock.shr_lock();
        unsafe { Self::from_raw(lock) }
    }

    /// Try to create a new `RawShareGuard`
    ///
    /// This function is non-blocking and may not panic
    pub fn try_new(lock: &'a L) -> Option<Self> {
        if lock.shr_try_lock() {
            Some(unsafe { Self::from_raw(lock) })
        } else {
            None
        }
    }
}

impl<'a, L: RawShareLock + RawLockInfo> RawShareGuard<'a, L> {
    /// Temporarily yields the lock to another thread if there is one.
    /// [read more](RawShareLock#method.shr_bump)
    pub fn bump(&mut self) {
        unsafe {
            self.lock.shr_bump();
        }
    }

    /// Temporarily unlocks the lock to execute the given function.
    ///
    /// This is safe because &mut guarantees that there exist no other references to the data protected by the lock.
    pub fn unlocked<R>(&mut self, f: impl FnOnce() -> R) -> R {
        unsafe {
            self.lock.shr_unlock();
        }
        defer!(self.lock.shr_lock());
        f()
    }

    pub fn inner(&self) -> &L {
        self.lock
    }

    pub fn into_inner(self) -> &'a L {
        std::mem::ManuallyDrop::new(self).lock
    }
}

impl<L: RawShareLockFair + RawLockInfo> RawShareGuard<'_, L> {
    /// Unlocks the guard using a fair unlocking protocol
    /// [read more](RawShareLockFair#method.shr_unlock_fair)
    pub fn unlock_fair(self) {
        let g = std::mem::ManuallyDrop::new(self);
        unsafe {
            g.lock.shr_unlock_fair();
        }
    }

    /// Temporarily yields the lock to a waiting thread if there is one.
    /// [read more](RawShareLockFair#method.shr_bump_fair)
    pub fn bump_fair(&mut self) {
        unsafe {
            self.lock.shr_bump_fair();
        }
    }

    /// Temporarily unlocks the lock to execute the given function.
    ///
    /// The lock is unlocked a fair unlock protocol.
    ///
    /// This is safe because `&mut` guarantees that there exist no other references to the data protected by the lock.
    pub fn unlocked_fair<R>(&mut self, f: impl FnOnce() -> R) -> R {
        unsafe {
            self.lock.shr_unlock_fair();
        }
        defer!(self.lock.shr_lock());
        f()
    }
}

impl<'a, L: RawShareLock + RawLockInfo> Clone for RawShareGuard<'a, L> {
    fn clone(&self) -> Self {
        unsafe {
            self.lock.shr_split();
            RawShareGuard {
                lock: self.lock,
                _traits: self._traits,
            }
        }
    }
}
