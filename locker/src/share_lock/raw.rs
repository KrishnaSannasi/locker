use super::RawShareLock;
use crate::RawLockInfo;

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

impl<'a, L: RawShareLock + RawLockInfo> RawShareGuard<'a, L> {
    /// # Safety
    ///
    /// The share lock must be held
    pub unsafe fn from_raw_parts(lock: &'a L, _traits: L::ShareGuardTraits) -> Self {
        Self { lock, _traits }
    }

    pub fn new(lock: &'a L, _traits: L::ShareGuardTraits) -> Self {
        lock.shr_lock();

        unsafe { Self::from_raw_parts(lock, _traits) }
    }

    pub fn try_new(lock: &'a L, _traits: L::ShareGuardTraits) -> Option<Self> {
        if lock.shr_try_lock() {
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

impl<'a, L: RawShareLock + RawLockInfo> Clone for RawShareGuard<'a, L> {
    fn clone(&self) -> Self {
        unsafe {
            self.lock.shr_split();
            RawShareGuard::from_raw_parts(self.lock, self._traits)
        }
    }
}
