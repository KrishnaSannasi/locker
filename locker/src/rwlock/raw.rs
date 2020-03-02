use super::RawRwLock;
use crate::exclusive_lock::RawExclusiveGuard;
use crate::share_lock::RawShareGuard;

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
    /// # Safety
    ///
    /// You must pass `RawUniueLock::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw(lock: L) -> Self {
        Self { lock }
    }

    #[inline]
    pub fn into_inner(self) -> L {
        self.lock
    }

    #[inline]
    pub const fn inner(&self) -> &L {
        &self.lock
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn inner_mut(&mut self) -> &mut L {
                &mut self.lock
            }
        } else {
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
            #[inline]
            pub const fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        } else {
            #[inline]
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
    #[inline]
    pub fn write(&self) -> RawExclusiveGuard<'_, L> {
        unsafe {
            self.lock.exc_lock();
            RawExclusiveGuard::from_raw(&self.lock)
        }
    }

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

    #[inline]
    pub fn read(&self) -> RawShareGuard<'_, L> {
        unsafe {
            self.lock.shr_lock();
            RawShareGuard::from_raw(&self.lock)
        }
    }

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
