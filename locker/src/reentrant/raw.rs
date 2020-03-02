use super::RawReentrantMutex;
use crate::share_lock::RawShareGuard;

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
    pub fn lock(&self) -> RawShareGuard<'_, L> {
        unsafe {
            self.lock.shr_lock();
            RawShareGuard::from_raw(&self.lock)
        }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<RawShareGuard<'_, L>> {
        unsafe {
            if self.lock.shr_try_lock() {
                Some(RawShareGuard::from_raw(&self.lock))
            } else {
                None
            }
        }
    }
}
