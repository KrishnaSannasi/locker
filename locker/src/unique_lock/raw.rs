use super::{RawUniqueLock, RawUniqueLockFair, SplittableUniqueLock};
use crate::RawLockInfo;

pub type RawUniqueGuard<'a, L> = _RawUniqueGuard<'a, L, <L as RawLockInfo>::UniqueGuardTraits>;
pub struct _RawUniqueGuard<'a, L: RawUniqueLock, Tr> {
    lock: &'a L,
    _traits: Tr,
}

impl<'a, L: RawUniqueLock, Tr> Drop for _RawUniqueGuard<'_, L, Tr> {
    fn drop(&mut self) {
        unsafe { self.lock.uniq_unlock() }
    }
}

impl<'a, L: RawUniqueLock + RawLockInfo> RawUniqueGuard<'a, L> {
    /// # Safety
    ///
    /// The unique lock must be held
    pub unsafe fn from_raw_parts(lock: &'a L, _traits: L::UniqueGuardTraits) -> Self {
        Self { lock, _traits }
    }

    pub fn new(lock: &'a L, _traits: L::UniqueGuardTraits) -> Self {
        lock.uniq_lock();

        unsafe { Self::from_raw_parts(lock, _traits) }
    }

    pub fn try_new(lock: &'a L, _traits: L::UniqueGuardTraits) -> Option<Self> {
        if lock.uniq_try_lock() {
            unsafe { Some(Self::from_raw_parts(lock, _traits)) }
        } else {
            None
        }
    }

    pub fn bump(&mut self) {
        unsafe { self.lock.uniq_bump(); }
    }

    pub fn unlocked<R>(&mut self, f: impl FnOnce() -> R) -> R {
        unsafe { self.lock.uniq_unlock(); }
        defer!(self.lock.uniq_lock());
        f()
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn inner(&self) -> &L {
        self.lock
    }
}

impl<L: RawUniqueLockFair + RawLockInfo> RawUniqueGuard<'_, L> {
    pub fn unlock_fair(self) {
        let g = std::mem::ManuallyDrop::new(self);
        unsafe { g.lock.uniq_unlock_fair(); }
    }
    
    pub fn bump_fair(&mut self) {
        unsafe { self.lock.uniq_bump_fair(); }
    }
    
    pub fn unlocked_fair<R>(&mut self, f: impl FnOnce() -> R) -> R {
        unsafe { self.lock.uniq_unlock_fair(); }
        defer!(self.lock.uniq_lock());
        f()
    }
}

impl<L: SplittableUniqueLock + RawLockInfo> Clone for RawUniqueGuard<'_, L> {
    fn clone(&self) -> Self {
        unsafe {
            self.lock.uniq_split();
            RawUniqueGuard::from_raw_parts(self.lock, self._traits)
        }
    }
}
