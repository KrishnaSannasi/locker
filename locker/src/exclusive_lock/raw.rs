use super::{
    RawExclusiveLock, RawExclusiveLockDowngrade, RawExclusiveLockFair, SplittableExclusiveLock,
};
use crate::{Inhabitted, RawLockInfo};

/// A RAII implementation of a scoped exclusive lock
///
/// This type represents a *exc lock*, and while it is alive there is an active *exc lock*
///
/// Once this structure is dropped, that *exc lock* will automatically be released by calling
/// [`RawExclusiveLock::exc_unlock`]. If you want to release the *exc lock* using a fair unlock
/// protocol, use [`RawExclusiveGuard::unlock_fair`](crate::exclusive_lock::RawExclusiveGuard#method.unlock_fair)
pub type RawExclusiveGuard<'a, L> =
    _RawExclusiveGuard<'a, L, <L as RawLockInfo>::ExclusiveGuardTraits>;

#[doc(hidden)]
#[must_use = "if unused the `RawExclusiveGuard` will immediately unlock"]
pub struct _RawExclusiveGuard<'a, L: RawExclusiveLock + ?Sized, Tr> {
    lock: &'a L,
    _traits: Tr,
}

impl<'a, L: RawExclusiveLock + ?Sized, Tr> Drop for _RawExclusiveGuard<'_, L, Tr> {
    fn drop(&mut self) {
        unsafe { self.lock.exc_unlock() }
    }
}

impl<'a, L: RawExclusiveLock + RawLockInfo + ?Sized> RawExclusiveGuard<'a, L>
where
    L::ExclusiveGuardTraits: Inhabitted,
{
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// # Safety
            ///
            /// An *exc lock* must owned for the given `lock`
            pub const unsafe fn from_raw(lock: &'a L) -> Self {
                Self { lock, _traits: Inhabitted::INIT }
            }
        } else {
            /// # Safety
            ///
            /// An *exc lock* must owned for the given `lock`
            pub unsafe fn from_raw(lock: &'a L) -> Self {
                Self { lock, _traits: Inhabitted::INIT }
            }
        }
    }

    /// Create a new `RawExclusiveGuard`
    ///
    /// blocks until lock is acquired
    ///
    /// # Panic
    ///
    /// This function may panic if the lock is cannot be acquired
    pub fn new(lock: &'a L) -> Self {
        lock.exc_lock();
        unsafe { Self::from_raw(lock) }
    }

    /// Try to create a new `RawExclusiveGuard`
    ///
    /// This function is non-blocking and may not panic
    pub fn try_new(lock: &'a L) -> Option<Self> {
        if lock.exc_try_lock() {
            Some(unsafe { Self::from_raw(lock) })
        } else {
            None
        }
    }
}

impl<'a, L: RawExclusiveLock + RawLockInfo> RawExclusiveGuard<'a, L> {
    /// Temporarily yields the lock to another thread if there is one.
    /// [read more](RawExclusiveLock#method.exc_bump)
    pub fn bump(&mut self) {
        unsafe {
            self.lock.exc_bump();
        }
    }

    /// Temporarily unlocks the lock to execute the given function.
    ///
    /// This is safe because &mut guarantees that there exist no other references to the data protected by the lock.
    pub fn unlocked<R>(&mut self, f: impl FnOnce() -> R) -> R {
        unsafe {
            self.lock.exc_unlock();
        }
        defer!(self.lock.exc_lock());
        f()
    }

    /// The inner lock
    pub fn inner(&self) -> &L {
        self.lock
    }

    /// Consume the guard without releasing the lock
    pub fn into_inner(self) -> &'a L {
        std::mem::ManuallyDrop::new(self).lock
    }
}

impl<L: RawExclusiveLockFair + RawLockInfo> RawExclusiveGuard<'_, L> {
    /// Unlocks the guard using a fair unlocking protocol
    /// [read more](RawExclusiveLockFair#method.exc_unlock_fair)
    pub fn unlock_fair(self) {
        let g = std::mem::ManuallyDrop::new(self);
        unsafe {
            g.lock.exc_unlock_fair();
        }
    }

    /// Temporarily yields the lock to a waiting thread if there is one.
    /// [read more](RawExclusiveLockFair#method.exc_bump_fair)
    pub fn bump_fair(&mut self) {
        unsafe {
            self.lock.exc_bump_fair();
        }
    }

    /// Temporarily unlocks the lock to execute the given function.
    ///
    /// The lock is unlocked a fair unlock protocol.
    ///
    /// This is safe because `&mut` guarantees that there exist no other references to the data protected by the lock.
    pub fn unlocked_fair<R>(&mut self, f: impl FnOnce() -> R) -> R {
        unsafe {
            self.lock.exc_unlock_fair();
        }
        defer!(self.lock.exc_lock());
        f()
    }
}

impl<'a, L: RawExclusiveLockDowngrade + RawLockInfo> RawExclusiveGuard<'a, L>
where
    L::ShareGuardTraits: Inhabitted,
{
    /// Atomically downgrades a write lock into a read lock without allowing
    /// any writers to take exclusive access of the lock in the meantime.
    pub fn downgrade(self) -> crate::share_lock::RawShareGuard<'a, L> {
        self.into()
    }
}

impl<'a, L: RawExclusiveLockDowngrade + RawLockInfo> From<RawExclusiveGuard<'a, L>>
    for crate::share_lock::RawShareGuard<'a, L>
where
    L::ShareGuardTraits: Inhabitted,
{
    /// Atomically downgrades a write lock into a read lock without allowing
    /// any writers to take exclusive access of the lock in the meantime.
    fn from(g: RawExclusiveGuard<'a, L>) -> Self {
        let lock = g.into_inner();
        unsafe {
            lock.downgrade();
            crate::share_lock::RawShareGuard::from_raw(lock)
        }
    }
}

impl<L: SplittableExclusiveLock + RawLockInfo> Clone for RawExclusiveGuard<'_, L> {
    fn clone(&self) -> Self {
        unsafe {
            self.lock.exc_split();
            RawExclusiveGuard {
                lock: self.lock,
                _traits: self._traits,
            }
        }
    }
}
