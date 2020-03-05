use super::RawReentrantMutex;
use crate::share_lock::{RawShareGuard, RawShareLockTimed};

#[repr(C)]
pub struct ReentrantMutex<L> {
    lock: L,
}

impl<L: RawReentrantMutex> Default for ReentrantMutex<L> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<L> ReentrantMutex<L> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
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

impl<L: RawReentrantMutex> ReentrantMutex<L> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new() -> Self {
                unsafe { Self::from_raw_parts(L::INIT) }
            }
        } else {
            #[inline]
            pub fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        }
    }
}

impl<L: RawReentrantMutex> ReentrantMutex<L>
where
    L::ShareGuardTraits: crate::Inhabitted,
{
    #[inline]
    unsafe fn lock_unchecked(&self) -> RawShareGuard<'_, L> {
        RawShareGuard::from_raw(&self.lock)
    }

    #[inline]
    pub fn lock(&self) -> RawShareGuard<'_, L> {
        unsafe {
            self.lock.shr_lock();
            self.lock_unchecked()
        }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<RawShareGuard<'_, L>> {
        if self.lock.shr_try_lock() {
            unsafe { Some(self.lock_unchecked()) }
        } else {
            None
        }
    }
}

impl<L: RawReentrantMutex + RawShareLockTimed> ReentrantMutex<L>
where
    L::ShareGuardTraits: crate::Inhabitted,
{
    #[inline]
    pub fn try_lock_until(&self, instant: L::Instant) -> Option<RawShareGuard<'_, L>> {
        if self.lock.shr_try_lock_until(instant) {
            unsafe { Some(self.lock_unchecked()) }
        } else {
            None
        }
    }

    #[inline]
    pub fn try_lock_for(&self, duration: L::Duration) -> Option<RawShareGuard<'_, L>> {
        if self.lock.shr_try_lock_for(duration) {
            unsafe { Some(self.lock_unchecked()) }
        } else {
            None
        }
    }
}
