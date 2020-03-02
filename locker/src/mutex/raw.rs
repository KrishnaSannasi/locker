use crate::exclusive_lock::RawExclusiveGuard;
use crate::mutex::RawMutex;

#[repr(transparent)]
pub struct Mutex<L> {
    lock: L,
}

impl<L: RawMutex> Default for Mutex<L> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<L> Mutex<L> {
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

impl<L: RawMutex> Mutex<L> {
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

impl<L: RawMutex> Mutex<L>
where
    L::ExclusiveGuardTraits: crate::Inhabitted,
{
    #[inline]
    pub fn lock(&self) -> RawExclusiveGuard<'_, L> {
        unsafe {
            self.lock.exc_lock();
            RawExclusiveGuard::from_raw(&self.lock)
        }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<RawExclusiveGuard<'_, L>> {
        unsafe {
            if self.lock.exc_try_lock() {
                Some(RawExclusiveGuard::from_raw(&self.lock))
            } else {
                None
            }
        }
    }
}
