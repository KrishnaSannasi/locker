use super::{RawShareLock, RawShareLockFair};
use crate::{Inhabitted, RawLockInfo};

pub type RawShareGuard<'a, L> = _RawShareGuard<'a, L, <L as RawLockInfo>::ShareGuardTraits>;
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
        lock.shr_lock();

        unsafe { Self::from_raw(lock) }
    }

    pub fn try_new(lock: &'a L) -> Option<Self> {
        if lock.shr_try_lock() {
            unsafe { Some(Self::from_raw(lock)) }
        } else {
            None
        }
    }
}

impl<'a, L: RawShareLock + RawLockInfo> RawShareGuard<'a, L> {
    pub fn bump(&mut self) {
        unsafe {
            self.lock.shr_bump();
        }
    }

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
    pub fn unlock_fair(self) {
        let g = std::mem::ManuallyDrop::new(self);
        unsafe {
            g.lock.shr_unlock_fair();
        }
    }

    pub fn bump_fair(&mut self) {
        unsafe {
            self.lock.shr_bump_fair();
        }
    }

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
