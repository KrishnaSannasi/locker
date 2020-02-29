use super::{RawExclusiveLock, RawExclusiveLockFair, RawExclusiveLockDowngrade, SplittableExclusiveLock};
use crate::{RawLockInfo, Inhabitted};

pub type RawExclusiveGuard<'a, L> =
    _RawExclusiveGuard<'a, L, <L as RawLockInfo>::ExclusiveGuardTraits>;
pub struct _RawExclusiveGuard<'a, L: RawExclusiveLock, Tr> {
    lock: &'a L,
    _traits: Tr,
}

impl<'a, L: RawExclusiveLock, Tr> Drop for _RawExclusiveGuard<'_, L, Tr> {
    fn drop(&mut self) {
        unsafe { self.lock.exc_unlock() }
    }
}

impl<'a, L: RawExclusiveLock + RawLockInfo> RawExclusiveGuard<'a, L>
where
    L::ExclusiveGuardTraits: Inhabitted,
{
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// # Safety
            ///
            /// The share lock must be held
            pub const unsafe fn from_raw(lock: &'a L) -> Self {
                Self { lock, _traits: Inhabitted::INIT }
            }
        } else {
            /// # Safety
            ///
            /// The share lock must be held
            pub unsafe fn from_raw(lock: &'a L) -> Self {
                Self { lock, _traits: Inhabitted::INIT }
            }
        }
    }

    pub fn new(lock: &'a L) -> Self {
        lock.exc_lock();

        unsafe { Self::from_raw(lock) }
    }

    pub fn try_new(lock: &'a L) -> Option<Self> {
        if lock.exc_try_lock() {
            unsafe { Some(Self::from_raw(lock)) }
        } else {
            None
        }
    }
}

impl<'a, L: RawExclusiveLock + RawLockInfo> RawExclusiveGuard<'a, L> {
    pub fn bump(&mut self) {
        unsafe {
            self.lock.exc_bump();
        }
    }

    pub fn unlocked<R>(&mut self, f: impl FnOnce() -> R) -> R {
        unsafe {
            self.lock.exc_unlock();
        }
        defer!(self.lock.exc_lock());
        f()
    }

    pub fn inner(&self) -> &L {
        self.lock
    }

    pub fn into_inner(self) -> &'a L {
        std::mem::ManuallyDrop::new(self).lock
    }
}

impl<L: RawExclusiveLockFair + RawLockInfo> RawExclusiveGuard<'_, L> {
    pub fn unlock_fair(self) {
        let g = std::mem::ManuallyDrop::new(self);
        unsafe {
            g.lock.exc_unlock_fair();
        }
    }

    pub fn bump_fair(&mut self) {
        unsafe {
            self.lock.exc_bump_fair();
        }
    }

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
    pub fn downgrade(self) -> crate::share_lock::RawShareGuard<'a, L> {
        let g = std::mem::ManuallyDrop::new(self);
        unsafe {
            g.lock.downgrade();
            crate::share_lock::RawShareGuard::from_raw(g.lock)
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
