use super::{RawUniqueLock, SplittableUniqueLock};
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

    /// # Safety
    ///
    /// TODO
    pub unsafe fn inner(&self) -> &L {
        self.lock
    }
}

impl<'a, L: SplittableUniqueLock + RawLockInfo> Clone for RawUniqueGuard<'a, L> {
    fn clone(&self) -> Self {
        unsafe {
            self.lock.uniq_split();
            RawUniqueGuard::from_raw_parts(self.lock, self._traits)
        }
    }
}
